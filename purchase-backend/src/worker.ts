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

type Tier = 'personal' | 'family' | 'pro' | 'personal_founding' | 'family_founding' | 'pro_founding';
const TIER_PRICES: Record<Tier, { amount_cents: number; label: string }> = {
    // Standard tiers — active from launch day onward.
    personal:          { amount_cents:  8900, label: 'SnapIT Personal — one owner, unlimited devices they own, perpetual licence'                          },
    family:            { amount_cents: 14900, label: 'SnapIT Family Pack — up to 5 profiles in one household, perpetual licence'                            },
    pro:               { amount_cents: 24900, label: 'SnapIT Pro / Studio — freelance and small studio, extended commercial licence, perpetual'            },
    // Founding-customer offer — first 500 buyers of each tier, then removed from the checkout page.
    personal_founding: { amount_cents:  5900, label: 'SnapIT Personal · Founding customer — one owner, unlimited devices, perpetual licence'                },
    family_founding:   { amount_cents:  9900, label: 'SnapIT Family Pack · Founding customer — 5 profiles per household, perpetual licence'                 },
    pro_founding:      { amount_cents: 19900, label: 'SnapIT Pro / Studio · Founding customer — extended commercial licence, perpetual'                     },
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
            if (url.pathname === '/waitlist' && req.method === 'POST') {
                return await handleWaitlist(req, env);
            }
            if (url.pathname === '/checkout' && req.method === 'POST') {
                // Sales are paused until the R·02 scope ships. Return a friendly 503.
                return json({ error: 'sales_paused', message: 'SnapIT is in build. Join the waitlist at snapit.vfempire.com.' }, 503);
            }
            if (url.pathname === '/webhook/stripe' && req.method === 'POST') {
                return await handleStripeWebhook(req, env, ctx);
            }
            if (url.pathname === '/reissue' && req.method === 'POST') {
                return await handleReissue(req, env);
            }
            if (url.pathname.startsWith('/updates/')) {
                // Keep serving update manifests so existing v0.1.6+ users can
                // still receive future patches. Existing customers > website noise.
                return await handleUpdateCheck(req, env, url);
            }
            if (url.pathname.startsWith('/downloads/')) {
                // Downloads paused until R·02 is complete. Redirect to the
                // waitlist landing so anyone hitting a bookmarked link lands
                // somewhere useful instead of a dead URL.
                return new Response(null, {
                    status: 302,
                    headers: {
                        location: 'https://snapit.vfempire.com/#waitform',
                        'cache-control': 'no-store',
                        ...CORS,
                    },
                });
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
// /waitlist — early-access email capture. Stored in KV keyed by email
// so re-submits are idempotent. R·02 open-day: dump the list, mail
// everyone a real download link.
// -------------------------------------------------------------------------

async function handleWaitlist(req: Request, env: Env): Promise<Response> {
    const body = await req.json<{ email?: string; product?: string }>().catch(() => ({}));
    const email = (body.email || '').trim().toLowerCase();
    const product = (body.product || 'snapit').slice(0, 32);
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) || email.length > 254) {
        return json({ error: 'invalid_email' }, 400);
    }
    // Store keyed by email so duplicates collapse. Also stamp signed_up_at
    // for the eventual export.
    await env.LICENCES.put(
        `waitlist:${product}:${email}`,
        JSON.stringify({ email, product, signed_up_at: new Date().toISOString() }),
    );
    return json({ ok: true });
}

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
//
// Tauri v2's updater plugin calls this endpoint and expects one of:
//   - 204 No Content  → nothing to install
//   - 200 with a JSON body: { version, pub_date, platforms: { "<target>-<arch>": { url, signature } } }
//
// We resolve `latest` from the GitHub Releases API of the source repo. The
// release's .sig files (produced by cargo-tauri build under the minisign key)
// carry the signature that the client verifies against the pubkey baked into
// its binary. No trust in this Worker — the Worker just points at signed
// artifacts.

const GH_REPO = 'vfempire-hq/snapit-desktop';
const UPDATE_CACHE_TTL_S = 300; // 5 min — fresh enough, cheap on GH API budget

async function handleUpdateCheck(req: Request, env: Env, url: URL): Promise<Response> {
    // parts: '', 'updates', target, arch, current_version
    const parts = url.pathname.split('/');
    if (parts.length < 5) return new Response(null, { status: 204 });
    const [, , tauriTarget, tauriArch, current] = parts;

    const cacheKey = new Request(new URL('/__cache/latest-release', url.origin).toString(), req);
    const cache = (caches as any).default as Cache | undefined;
    let releaseJson: any | null = null;

    if (cache) {
        const hit = await cache.match(cacheKey);
        if (hit) {
            try { releaseJson = await hit.json(); } catch { /* fall through */ }
        }
    }

    if (!releaseJson) {
        const gh = await fetch(`https://api.github.com/repos/${GH_REPO}/releases/latest`, {
            headers: {
                'user-agent': 'snapit-updater/1',
                'accept': 'application/vnd.github+json',
            },
        });
        if (!gh.ok) return new Response(null, { status: 204 });
        releaseJson = await gh.json<any>();
        if (cache) {
            const cachedCopy = new Response(JSON.stringify(releaseJson), {
                headers: {
                    'content-type': 'application/json',
                    'cache-control': `public, max-age=${UPDATE_CACHE_TTL_S}`,
                },
            });
            await cache.put(cacheKey, cachedCopy);
        }
    }

    const latestVersion = (releaseJson.tag_name || '').replace(/^v/, '');
    if (!latestVersion || compareSemver(latestVersion, current) <= 0) {
        return new Response(null, { status: 204 });
    }

    const platformKey = `${normaliseTauriTarget(tauriTarget)}-${tauriArch}`;
    const matcher = platformMatchers[platformKey];
    if (!matcher) return new Response(null, { status: 204 });

    const assets: Array<{ name: string; browser_download_url: string }> = releaseJson.assets || [];
    const installer = assets.find((a) => matcher.installer.test(a.name));
    if (!installer) return new Response(null, { status: 204 });

    // Signature file is either "<installer>.sig" or the .app.tar.gz.sig for macOS.
    const sigAsset = assets.find((a) => a.name === `${installer.name}.sig`);
    if (!sigAsset) return new Response(null, { status: 204 });

    const sig = await fetch(sigAsset.browser_download_url).then((r) => (r.ok ? r.text() : ''));
    if (!sig) return new Response(null, { status: 204 });

    const body = {
        version: latestVersion,
        pub_date: releaseJson.published_at || new Date().toISOString(),
        notes: (releaseJson.body || '').slice(0, 4000),
        platforms: {
            [platformKey]: {
                url: installer.browser_download_url,
                signature: sig.trim(),
            },
        },
    };
    return json(body);
}

// Tauri emits `darwin` / `linux` / `windows` as target and arch as
// `aarch64` / `x86_64` / `armv7`. GH assets from `tauri build` land with
// specific names per platform — regexes below map to those exact names.
const platformMatchers: Record<string, { installer: RegExp }> = {
    'linux-x86_64':   { installer: /\.AppImage$/ },
    'windows-x86_64': { installer: /-setup\.exe$/ },
    'darwin-x86_64':  { installer: /\.app\.tar\.gz$/ },
    'darwin-aarch64': { installer: /\.app\.tar\.gz$/ },
};

function normaliseTauriTarget(t: string): string {
    if (t.startsWith('darwin')) return 'darwin';
    if (t.startsWith('linux')) return 'linux';
    if (t.startsWith('windows')) return 'windows';
    return t;
}

function compareSemver(a: string, b: string): number {
    const [aa, bb] = [a, b].map((v) => v.split('.').map((n) => parseInt(n, 10) || 0));
    for (let i = 0; i < 3; i++) {
        const d = (aa[i] ?? 0) - (bb[i] ?? 0);
        if (d !== 0) return d;
    }
    return 0;
}

// -------------------------------------------------------------------------
// /downloads/* → resolve platform shortcuts to the latest signed GH Release
//               asset (and proxy install.sh / install.ps1 from R2 if bound).
// -------------------------------------------------------------------------

const PLATFORM_ASSET_RE: Record<string, RegExp> = {
    'mac-arm64':       /_aarch64\.dmg$/,
    'mac-intel':       /_x64\.dmg$/,
    'win-setup':       /_x64-setup\.exe$/,
    'win-msi':         /_x64_en-US\.msi$/,
    'linux-appimage':  /_amd64\.AppImage$/,
    'linux-deb':       /_amd64\.deb$/,
};

async function handleDownload(req: Request, env: Env, url: URL): Promise<Response> {
    const key = url.pathname.slice('/downloads/'.length);

    // Platform shortcut → 302 to the latest matching asset on the latest release.
    const platMatch = PLATFORM_ASSET_RE[key];
    if (platMatch) {
        const cache = (caches as any).default as Cache | undefined;
        const cacheKey = new Request(new URL('/__cache/latest-release', url.origin).toString(), req);
        let releaseJson: any | null = null;
        if (cache) {
            const hit = await cache.match(cacheKey);
            if (hit) {
                try { releaseJson = await hit.json(); } catch {}
            }
        }
        if (!releaseJson) {
            const gh = await fetch(`https://api.github.com/repos/${GH_REPO}/releases/latest`, {
                headers: { 'user-agent': 'snapit-downloads/1', accept: 'application/vnd.github+json' },
            });
            if (!gh.ok) return new Response('release lookup failed', { status: 502 });
            releaseJson = await gh.json<any>();
            if (cache) {
                await cache.put(cacheKey, new Response(JSON.stringify(releaseJson), {
                    headers: { 'content-type': 'application/json', 'cache-control': `public, max-age=${UPDATE_CACHE_TTL_S}` },
                }));
            }
        }
        const assets: Array<{ name: string; browser_download_url: string }> = releaseJson.assets || [];
        const hit = assets.find((a) => platMatch.test(a.name));
        if (!hit) return new Response('no matching artifact on latest release', { status: 404 });
        return new Response(null, {
            status: 302,
            headers: {
                location: hit.browser_download_url,
                'cache-control': 'public, max-age=60',
                ...CORS,
            },
        });
    }

    // Everything else (install.sh, install.ps1) — proxy to R2 if bound.
    if (!env.INSTALLERS) return new Response('R2 not bound yet', { status: 503 });
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
