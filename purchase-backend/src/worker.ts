// SnapIT purchase-backend (Cloudflare Worker).
//
// Two live endpoints for R·01 M6:
//   POST /checkout          create a Stripe Checkout session for a tier
//   POST /webhook/stripe    handle checkout.session.completed → issue licence
//   POST /reissue           re-email the licence from the Stripe session id
//                           (customer support lane for lost keys)
//
// Two live endpoints for R·01 M4 (auto-updater):
//   GET  /updates/:target/:arch/:current_version
//   GET  /downloads/*       proxy to R2 for installer files
//
// Every licence blob is signed with an Ed25519 key held in a Wrangler
// secret. The public key is baked into the SnapIT binary at build time
// (see app/src-tauri/src/licence/mod.rs).

import Stripe from 'stripe';

type Env = {
    // KV
    LICENCES: KVNamespace;
    // R2 for prebuilt installers
    INSTALLERS?: R2Bucket;
    // Static assets
    ASSETS: Fetcher;

    // Secrets (Wrangler-managed)
    STRIPE_SECRET_KEY: string;
    STRIPE_WEBHOOK_SECRET: string;
    LICENCE_SIGNING_KEY: string;  // hex-encoded 32-byte Ed25519 seed
    VFMAIL_SMTP_TOKEN?: string;   // for licence delivery via VF Mail

    // Vars
    PRODUCT_NAME: string;
    SUPPORT_EMAIL: string;
    LICENCE_ISSUER: string;
};

type Tier = 'personal' | 'family' | 'pro';
const TIER_PRICES: Record<Tier, { amount_cents: number; label: string }> = {
    personal: { amount_cents: 6900, label: 'SnapIT Personal — one photographer, perpetual licence' },
    family:   { amount_cents: 12900, label: 'SnapIT Family — up to 5 devices in one household, perpetual licence' },
    pro:      { amount_cents: 12900, label: 'SnapIT Pro — freelance / small studio, perpetual licence' },
};

const CORS: Record<string, string> = {
    'access-control-allow-origin': '*',
    'access-control-allow-methods': 'GET,POST,OPTIONS',
    'access-control-allow-headers': 'content-type',
};

export default {
    async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
        const url = new URL(req.url);

        if (req.method === 'OPTIONS') {
            return new Response(null, { status: 204, headers: CORS });
        }

        try {
            if (url.pathname === '/checkout' && req.method === 'POST') {
                return await handleCheckout(req, env);
            }
            if (url.pathname === '/webhook/stripe' && req.method === 'POST') {
                return await handleStripeWebhook(req, env, ctx);
            }
            if (url.pathname === '/reissue' && req.method === 'POST') {
                return await handleReissue(req, env);
            }
            if (url.pathname.startsWith('/updates/')) {
                return await handleUpdateCheck(req, env, url);
            }
            if (url.pathname.startsWith('/downloads/')) {
                return await handleDownload(req, env, url);
            }
            // Fall through to static assets (product landing page for snapit.vfempire.com)
            return env.ASSETS.fetch(req);
        } catch (e: any) {
            console.error('worker error:', e?.message || e);
            return json({ error: 'internal', message: e?.message || String(e) }, 500);
        }
    },
};

// -------------------------------------------------------------------------
// /checkout
// -------------------------------------------------------------------------

async function handleCheckout(req: Request, env: Env): Promise<Response> {
    const body = await req.json<{ tier?: Tier; email?: string; success_url?: string; cancel_url?: string }>().catch(() => ({}));
    const tier: Tier = (body.tier as Tier) || 'personal';
    if (!TIER_PRICES[tier]) return json({ error: 'unknown_tier' }, 400);

    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { apiVersion: '2024-09-30.acacia' });
    const session = await stripe.checkout.sessions.create({
        mode: 'payment',
        payment_method_types: ['card'],
        customer_email: body.email,
        line_items: [
            {
                price_data: {
                    currency: 'eur',
                    product_data: {
                        name: TIER_PRICES[tier].label,
                        description: 'One-off perpetual licence. No subscription. Read the licence file at snapit.vfempire.com/licence.',
                    },
                    unit_amount: TIER_PRICES[tier].amount_cents,
                },
                quantity: 1,
            },
        ],
        metadata: {
            product: 'snapit',
            tier,
        },
        success_url: body.success_url ?? 'https://snapit.vfempire.com/thanks?session_id={CHECKOUT_SESSION_ID}',
        cancel_url:  body.cancel_url  ?? 'https://snapit.vfempire.com/',
        automatic_tax: { enabled: false }, // toggled on once VAT registration lands
    });

    return json({ id: session.id, url: session.url });
}

// -------------------------------------------------------------------------
// /webhook/stripe — checkout.session.completed → issue + persist a licence
// -------------------------------------------------------------------------

async function handleStripeWebhook(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { apiVersion: '2024-09-30.acacia' });
    const raw = await req.text();
    const sig = req.headers.get('stripe-signature') || '';
    let event: Stripe.Event;
    try {
        event = await stripe.webhooks.constructEventAsync(raw, sig, env.STRIPE_WEBHOOK_SECRET);
    } catch (e: any) {
        return new Response('invalid_signature: ' + (e?.message ?? String(e)), { status: 400 });
    }

    if (event.type !== 'checkout.session.completed') {
        return json({ ok: true, ignored: event.type });
    }

    const session = event.data.object as Stripe.Checkout.Session;
    const email =
        session.customer_details?.email ||
        session.customer_email ||
        (typeof session.customer === 'string' ? undefined : session.customer?.email) ||
        null;
    const tier = (session.metadata?.tier as Tier) || 'personal';

    if (!email) {
        console.warn('checkout.session.completed without email; cannot issue licence', session.id);
        return json({ ok: false, error: 'no_email' });
    }

    // Deterministic licence blob + signature
    const licence = {
        email: email.toLowerCase(),
        tier,
        issued_at: Math.floor(Date.now() / 1000),
        product: 'snapit',
        major_version: 1,
    };
    const licence_json = JSON.stringify(licence);
    const signature_b64 = await signLicence(licence_json, env.LICENCE_SIGNING_KEY);

    // Persist for reissue lane (keyed by Stripe session id)
    await env.LICENCES.put(
        `session:${session.id}`,
        JSON.stringify({ licence_json, signature_b64, stripe_session_id: session.id }),
        { metadata: { email, tier } },
    );
    // Also key by email so a user with a lost session id can request via /reissue
    await env.LICENCES.put(
        `email:${email.toLowerCase()}:${tier}`,
        JSON.stringify({ licence_json, signature_b64, stripe_session_id: session.id }),
    );

    // Fire-and-forget email delivery
    ctx.waitUntil(deliverLicenceEmail(env, email, licence, licence_json, signature_b64));

    return json({ ok: true });
}

// -------------------------------------------------------------------------
// /reissue — customer-support lane. Body: { stripe_session_id?, email? }
// -------------------------------------------------------------------------

async function handleReissue(req: Request, env: Env): Promise<Response> {
    const body = await req.json<{ stripe_session_id?: string; email?: string; tier?: Tier }>().catch(() => ({}));
    let record: string | null = null;

    if (body.stripe_session_id) {
        record = await env.LICENCES.get(`session:${body.stripe_session_id}`);
    }
    if (!record && body.email) {
        const tier = body.tier ?? 'personal';
        record = await env.LICENCES.get(`email:${body.email.toLowerCase()}:${tier}`);
    }
    if (!record) return json({ error: 'not_found' }, 404);

    const parsed = JSON.parse(record);
    const licence = JSON.parse(parsed.licence_json);
    // Re-deliver
    await deliverLicenceEmail(env, licence.email, licence, parsed.licence_json, parsed.signature_b64);
    return json({ ok: true, delivered_to: licence.email });
}

// -------------------------------------------------------------------------
// /updates/:target/:arch/:current_version → signed update manifest
// -------------------------------------------------------------------------

async function handleUpdateCheck(_req: Request, _env: Env, url: URL): Promise<Response> {
    // parts: '', 'updates', target, arch, current_version
    const parts = url.pathname.split('/');
    if (parts.length < 5) return new Response(null, { status: 204 });
    // In R·01 we return 204 (no update) until we ship the release-manifest job.
    // R·02 populates a KV entry per target+arch pointing at the latest signed
    // installer in R2 and returns a JSON manifest with URL + signature.
    return new Response(null, { status: 204 });
}

// -------------------------------------------------------------------------
// /downloads/* → proxy to R2
// -------------------------------------------------------------------------

async function handleDownload(_req: Request, env: Env, url: URL): Promise<Response> {
    if (!env.INSTALLERS) return new Response('R2 not bound yet', { status: 503 });
    const key = url.pathname.slice('/downloads/'.length);
    const obj = await env.INSTALLERS.get(key);
    if (!obj) return new Response('not found', { status: 404 });
    const headers = new Headers();
    obj.writeHttpMetadata(headers);
    headers.set('content-disposition', `attachment; filename="${key.split('/').pop()}"`);
    return new Response(obj.body, { headers });
}

// -------------------------------------------------------------------------
// helpers
// -------------------------------------------------------------------------

function json(body: any, status = 200): Response {
    return new Response(JSON.stringify(body), {
        status,
        headers: { 'content-type': 'application/json', ...CORS },
    });
}

async function signLicence(licence_json: string, seed_hex: string): Promise<string> {
    const seed = hexToBytes(seed_hex);
    if (seed.length !== 32) throw new Error('LICENCE_SIGNING_KEY must be 32-byte hex');
    const key = await crypto.subtle.importKey('raw', seed, { name: 'Ed25519' }, false, ['sign']);
    const sig = new Uint8Array(await crypto.subtle.sign('Ed25519', key, new TextEncoder().encode(licence_json)));
    return btoa(String.fromCharCode(...sig));
}

function hexToBytes(hex: string): Uint8Array {
    const s = hex.startsWith('0x') ? hex.slice(2) : hex;
    const out = new Uint8Array(s.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(s.substr(i * 2, 2), 16);
    return out;
}

async function deliverLicenceEmail(
    env: Env,
    to: string,
    licence: { email: string; tier: Tier; issued_at: number },
    licence_json: string,
    signature_b64: string,
): Promise<void> {
    // R·01 M6a: log-only. The actual VF Mail SMTP wiring lands with the VF Mail
    // "outbound API for other VF products" wave. Once that endpoint exists we
    // POST here to https://mail.vfempire.com/api/outbound with a Bearer token.
    console.log(`[snapit] would email ${to}: tier=${licence.tier}, licence.json len=${licence_json.length}, sig len=${signature_b64.length}`);
    // Placeholder — replace with:
    // await fetch('https://mail.vfempire.com/api/outbound', {
    //   method: 'POST',
    //   headers: {
    //     'authorization': `Bearer ${env.VFMAIL_SMTP_TOKEN}`,
    //     'content-type': 'application/json',
    //   },
    //   body: JSON.stringify({
    //     from: 'licence@vfempire.com',
    //     to,
    //     subject: `Your SnapIT ${licence.tier} licence`,
    //     text: buildLicenceEmail(licence, licence_json, signature_b64, env),
    //     attachments: [
    //       { filename: 'snapit.licence.json', content: licence_json, contentType: 'application/json' },
    //       { filename: 'snapit.licence.sig',  content: signature_b64, contentType: 'text/plain' },
    //     ],
    //   }),
    // });
}
