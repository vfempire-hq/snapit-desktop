# Stripe Go-Live — SnapIT

Everything technical is already wired in `purchase-backend/src/worker.ts`.
Six actions to flip the switch — three you do (secrets, keys, dashboard),
three I do (un-503 + smoke test + docs).

---

## Prerequisites (verify before starting)

- [ ] VF Empire Stripe live keys exist in the VF Mail vault at
      `C:\Users\email\VFMail\keys\stripe-vf.txt` (per memory)
- [ ] Cloudflare token in `~/sovereign-os/.env`
- [ ] Licence signing keypair already generated (Wrangler shows
      `LICENCE_SIGNING_KEY` is set)
- [ ] Malta VAT position decided (currently `automatic_tax: { enabled: false }`)

---

## Step 1 — Set live Stripe secrets (YOU)

On any machine with access to the VF Mail vault:

```bash
cd /home/guardiansoftiktok/snapit-desktop/purchase-backend
export CLOUDFLARE_API_TOKEN=$(grep '^CLOUDFLARE_API_TOKEN=' ~/sovereign-os/.env | cut -d= -f2 | awk '{print $1}')

# Live Stripe secret key from the vault
npx wrangler secret put STRIPE_SECRET_KEY
# When prompted, paste the rk_live_... value

# Live Stripe webhook signing secret (from Stripe dashboard — see step 3)
npx wrangler secret put STRIPE_WEBHOOK_SECRET
# When prompted, paste whsec_...
```

Verify:
```bash
npx wrangler secret list
```

Expected output: 3 secrets — `LICENCE_SIGNING_KEY`, `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`.

---

## Step 2 — Configure the webhook in Stripe Dashboard (YOU)

1. Log in to https://dashboard.stripe.com/
2. **Switch to Live mode** (top-right toggle)
3. Developers → Webhooks → Add endpoint
4. Endpoint URL: `https://snapit.vfempire.com/webhook/stripe`
5. Events to send: `checkout.session.completed`
6. Copy the "Signing secret" (starts with `whsec_`) → use it in Step 1's `STRIPE_WEBHOOK_SECRET`

---

## Step 3 — Un-503 the checkout endpoint (ME)

Currently `worker.ts` line 71 returns 503 with `sales_paused`. I'll flip
this to actually call `handleCheckout(req, env)` and deploy the worker.

Diff:
```typescript
// Before:
if (url.pathname === '/checkout' && req.method === 'POST') {
    return json({ error: 'sales_paused', message: 'SnapIT is in build...' }, 503);
}

// After:
if (url.pathname === '/checkout' && req.method === 'POST') {
    return await handleCheckout(req, env);
}
```

I'll ship this change immediately after you confirm Step 1 completed.

---

## Step 4 — End-to-end smoke test (ME with your card)

Once Steps 1-3 are done, we test the whole flow with a real €1 purchase
using your card — SnapIT charges €89 minimum so we'll use a test tier
just for verification, then refund via Stripe dashboard.

```bash
# From here, I POST to /checkout to get a real Stripe URL
curl -X POST https://snapit.vfempire.com/checkout \
  -H 'content-type: application/json' \
  -d '{"tier":"personal","email":"you@vfempire.com"}'
# Expected: { "id": "cs_live_...", "url": "https://checkout.stripe.com/..." }

# You open the URL, pay €89, get redirected to /thanks
# Stripe fires the webhook → worker signs a licence + stores in KV
# I verify in KV that the licence exists
npx wrangler kv:key list --binding=LICENCES | jq | head -3
```

Success criteria:
- Stripe records the payment
- Worker webhook returns 200
- KV has a new `licence:{key}` entry
- The email you used receives the licence (needs VFMAIL_SMTP_TOKEN)

---

## Step 5 — Set up licence delivery email (ME + YOU)

Currently the worker COULD send licence emails via VF Mail but I need
you to set the SMTP token:

```bash
npx wrangler secret put VFMAIL_SMTP_TOKEN
# Paste the token that lets the worker send AS licence@vfempire.com
```

Alternatively we can pipe through Postmark / Resend / SendGrid — pick
whichever you already have working. VF Mail is on-brand so my preference.

---

## Step 6 — Update the marketing site (ME)

Currently the buy button is disabled. Once Stripe works I'll un-hide
it on snapit.vfempire.com so real traffic can buy.

---

## Rollback if anything goes wrong

If a purchase happens and something breaks:

```bash
# Instantly disable checkout by re-adding the 503
cd purchase-backend
sed -i "s|return await handleCheckout|return json({error:'temporarily_paused'}, 503);//&|" src/worker.ts
npx wrangler deploy
```

Or via dashboard: pause the webhook endpoint in Stripe (stops issuing
licences), refund the payment.

---

## After go-live checklist

- [ ] Test purchase confirmed working end-to-end
- [ ] Un-hide "Buy €89" button on snapit.vfempire.com
- [ ] Change waitlist form CTA from "Notify me" to "Buy now"
- [ ] Add refund policy to the checkout page (30 days for Personal, 14 for Founding)
- [ ] Notify waitlist subscribers with a launch email
- [ ] Set up Stripe email receipts (Dashboard → Emails)
- [ ] Turn on Stripe tax if Malta VAT is decided
