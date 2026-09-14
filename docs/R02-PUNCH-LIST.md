# SnapIT R·02 — Public Launch Punch List

**Status when written:** R·01 (mock + marketing site) is live at snapit.vfempire.com/preview-x8f2r7/. Sales paused. Downloads paused. This document is the scope for shipping a real, paid, downloadable v1.0.

**Two-track approach recommended:**
- **R·02a MVP** — Tauri wrap + indexer + paid + signed. Ships in ~4 weeks. Charges money.
- **R·02b Full** — Every feature the mock demoes lands. Ships ~4 more weeks after MVP.

Ship MVP, start revenue, use it to fund parity work.

---

## R·02a — MVP (4 weeks, must-have to charge money)

### 1 · Tauri v2 shell (3–4 days)
- [ ] Wrap the existing `mock/index.html` in Tauri v2 so it opens as a native window
- [ ] Set up `src-tauri/` with proper bundle config (macOS/Windows/Linux)
- [ ] Wire the mock's JS calls (`window.ProfileStore`) to real Tauri commands
- [ ] Native menu bar (macOS/Linux), Windows menu (File/Edit/View/Help)
- [ ] Dock/taskbar icon + About dialog
- [ ] Deep-link handler for `snapit://` URLs (share links open in-app)
- [ ] File-association: double-click a photo → "Open with SnapIT"

**Depends on:** nothing. Can start immediately.

### 2 · Rust indexer (1 week)
- [ ] `snapit-indexer` crate — walks directories recursively with async I/O
- [ ] EXIF read via `kamadak-exif` — date, GPS, camera, lens, orientation, ISO
- [ ] Perceptual hash via `img-hash` (pHash) for dedup
- [ ] Thumbnail extraction — 256px + 1024px, cached to `~/.snapit/thumbs/`
- [ ] SQLite catalog via `rusqlite` — schema in `docs/LIBRARY-FORMAT.md`
- [ ] File-watcher via `notify` crate — new files land automatically
- [ ] Progress events streamed to the UI via Tauri events

**Depends on:** #1

### 3 · Source manager (2 days)
- [ ] Native filesystem-scan for the three onboarding phases (auto-scan / network / phones)
- [ ] macOS: `system_profiler SPCameraDataType`, iCloud Drive detection, Photos library `.photoslibrary` parsing
- [ ] Windows: `Get-PnpDevice` for phones, OneDrive detection, mapped drives
- [ ] Linux: `udisksctl` for mounted drives
- [ ] SMB/AFP/NFS scan via `mdns` crate for phase B
- [ ] iOS/Android phone read via libimobiledevice / MTP

**Depends on:** #1, #2

### 4 · Profiles + auth (2 days — Rust side already scaffolded)
- [ ] Wire the existing `app/src-tauri/src/profile/mod.rs` (Argon2id already there) to Tauri commands
- [ ] Replace `mock/store.js` LocalStorage-backed store with Tauri `invoke` calls
- [ ] Family Pack seat limit enforced in Rust
- [ ] PIN prompt UI on Tauri app boot
- [ ] Migration script — imports any existing `snapit_profiles_v1` localStorage into SQLite

**Depends on:** #1

### 5 · Licensing end-to-end (3 days)
- [ ] Un-503 the `/checkout` endpoint — Stripe live keys from the VF vault
- [ ] Stripe webhook already exists — smoke-test full flow with a real €1 test purchase
- [ ] Ed25519 keypair generated + public key baked into the Tauri app
- [ ] Machine binding — OS-specific fingerprint (macOS `system_profiler UUID`, Windows `MachineGuid`, Linux `/etc/machine-id`)
- [ ] Licence file written to OS keychain, verified on every launch
- [ ] Seat manager page (`/seats`) already built — connect the "Kick machine" action to a real endpoint
- [ ] Grace period — 7 days offline before licence check fails
- [ ] Reissue flow tested via `/reissue`

**Depends on:** #1

### 6 · Code signing + notarisation (blocked, needs unblocking first)
- [ ] **macOS**: Apple Developer ID cert (task #40 — verify API key access)
- [ ] **Windows**: Azure Trusted Signing (blocked — verification-code emails not arriving per memory `project_azure_signing_blocked_2026_09_13`)
- [ ] **Linux**: AppImage signing via Sigstore, .deb signing via GPG
- [ ] GitHub Actions matrix — one runner per platform, artefact uploads
- [ ] Signed manifest for the Tauri auto-updater
- [ ] rDNS-verified update host: `updates.snapit.vfempire.com` → Cloudflare Worker

**Depends on:** #5 · **BLOCKED ON** Azure signup verification (Vincent side)

### 7 · Downloads page live (1 day)
- [ ] Un-302 the `/downloads/*` handler in the purchase worker
- [ ] Downloads page on snapit.vfempire.com with proper platform detect + version picker
- [ ] SHA-256 checksums published alongside every artefact
- [ ] Release notes page (`/releases/vX.Y.Z`)

**Depends on:** #6

### 8 · Legal (parallel to build, 3 days)
- [ ] Terms of Service — SnapIT specific (not VF generic)
- [ ] EULA — perpetual licence, seat limits, redistribution restrictions
- [ ] Privacy Policy — "everything on your machine" claim needs precise wording
- [ ] Refund policy — 30-day money-back on Personal, 14-day on Founding tiers
- [ ] Permanence Guarantee — already drafted, needs review + `/permanence` page live
- [ ] Cookie policy for the marketing site
- [ ] Legal review by Malta counsel (VF Empire Corp Ltd's jurisdiction)

**Depends on:** nothing. Start in parallel.

### 9 · Support + docs (2 days)
- [ ] Support inbox: `support@vfempire.com` already routes — add SnapIT tag
- [ ] FAQ page — top 10 questions from the beta waitlist responses
- [ ] Getting Started guide — same content as the 24-step in-app tour
- [ ] Troubleshooting page — "SmartScreen warning", "app won't open on Mac", "sources not detected"
- [ ] Contact form on the site → routes to support inbox

**Depends on:** nothing. Start in parallel.

### 10 · Cross-platform QA (3 days)
- [ ] Install matrix: macOS 12/13/14/15 · Windows 10/11 · Ubuntu 22.04/24.04
- [ ] Smoke tests: install → onboard → import 1000 photos → search → open detail → sign out → uninstall
- [ ] Performance floor: 100k photos indexed in <40min on baseline hardware (M1 Air / i5 laptop)
- [ ] Memory ceiling: <500MB RSS with 10k photos loaded
- [ ] Crash-report opt-in — reports go to VF's own Sentry, not a SaaS

**Depends on:** #1-#7

---

## R·02b — Feature parity with the mock (4 weeks after MVP)

### 11 · Feature wall + Home
- [ ] Port all 10 pro layouts to native rendering (currently CSS-only in the mock)
- [ ] Live billboard update from real event data (already done in the mock, just needs Rust event feed)
- [ ] "Featured event" auto-pick heuristic — starred + recent + not-yet-opened

### 12 · Semantic search
- [ ] CLIP ONNX model bundled — `openai/clip-vit-base-patch32` (~150MB)
- [ ] Embedding cache in SQLite (one row per photo, 512-dim vector)
- [ ] Query encoder runs on-device via `ort` (ONNX Runtime for Rust)
- [ ] KNN lookup via faiss or `hnsw_rs` for millisecond search

### 13 · Face grouping
- [ ] Face detection: RetinaFace or SCRFD ONNX (~50MB)
- [ ] Face embeddings: ArcFace ONNX (~90MB)
- [ ] Clustering: DBSCAN over cosine distance
- [ ] UI: rename cluster, merge, split, mark as "not a person" — all already in the mock
- [ ] Consent gate — off by default, prompted on second launch

### 14 · Import queue with real progress
- [ ] Wire the mock's animated queue to real events from the Rust indexer
- [ ] Pause/resume/cancel per source
- [ ] Auto-throttle when battery <30% on laptops

### 15 · Detail modal + Enhance
- [ ] Real-ESRGAN ONNX model bundled (~65MB for 2×, ~130MB for 4×)
- [ ] GFPGAN for face-focused enhance
- [ ] Non-destructive save: writes `<name>-enhanced.png` next to original
- [ ] Before/after wipe slider already coded — just needs real ESRGAN output plumbed in

### 16 · Cinematic intro
- [ ] Web Audio sting already synthesised in the mock, port to Tauri
- [ ] Video preview: use `wgpu` for the swell animation instead of CSS keyframes
- [ ] Trigger heuristic already coded (starred + long video)

### 17 · Share links
- [ ] Backend: `share.snapit.vfempire.com` Cloudflare Worker
- [ ] Encryption: ChaCha20-Poly1305 with a random 32-byte key
- [ ] Bundle format: signed manifest + encrypted photo blobs, stored in R2
- [ ] Key lives in URL fragment, viewer page decrypts client-side
- [ ] Revoke endpoint deletes the R2 bundle
- [ ] Password gate on top of the E2E key
- [ ] Real QR code generation (currently CSS-only)

### 18 · Socials imports
- [ ] Instagram/Facebook/YouTube OAuth flows
- [ ] Takeout ZIP parser for TikTok/Snapchat/BeReal/VSCO/Vero
- [ ] Platform metadata schema in SQLite (handle, caption, likes, music)
- [ ] Real IG/TT/YT chrome overlays (already ported to the mock)

### 19 · Cleanup engine
- [ ] Duplicate detector via pHash bucketing
- [ ] Blur detector — Laplacian variance below threshold
- [ ] Screenshot detector — resolution matches known device screen sizes
- [ ] Receipt/document detector — small on-device OCR + heuristic
- [ ] Bulk action: move to `~/.snapit/trash/` (recoverable) not delete

### 20 · Auto-updater
- [ ] Tauri v2 updater plugin
- [ ] Signed manifest served from `updates.snapit.vfempire.com`
- [ ] Delta updates for platforms where it works (macOS)
- [ ] Rollback flow for bad releases

### 21 · Reviews with anti-spam
- [ ] Rate limit `/reviews POST` (1/min/IP)
- [ ] Turnstile challenge on unverified reviews
- [ ] Moderation queue in the seat-manager admin page
- [ ] Reply-from-vendor feature

---

## R·02c — Nice-to-have, ship v1.1 (optional)

- Mobile companion (iOS/Android) — Vault Wi-Fi pair + camera roll
- Wrapped page (year-end recap)
- On This Day / Hidden Gems auto-surfacing
- Send-to-phone via local Wi-Fi
- Print & PDF album export
- Cast-to-TV (AirPlay + Chromecast)
- Photo diary (searchable notes)
- Memory Match game (Kids profile)
- Face-blur strangers batch action
- Moodboard section (per Vincent's earlier ask)

---

## Real timeline (honest estimates)

| Phase | Optimistic | Realistic | With unblocking |
|-------|-----------|-----------|-----------------|
| R·02a MVP | 3 weeks | 4 weeks | 5–6 weeks |
| R·02b parity | 4 weeks | 5–6 weeks | 6–8 weeks |
| **Total to full parity** | **7 weeks** | **9–10 weeks** | **11–14 weeks** |

**Realistic total from today: ~2.5 months to MVP + revenue, ~4 months to full parity.**

Ship MVP as v1.0. Feature-parity releases as v1.1 through v1.4 over the following weeks.

---

## Blockers Vincent needs to unblock

1. **Azure Trusted Signing** — verification codes not arriving. Retry each session or switch to DigiCert.
2. **Apple Developer API key** — task #40, needs verification.
3. **Malta legal counsel** — retainer for ToS/EULA review.
4. **Stripe live keys** — locate the VF rk_live_ per memory `reference_stripe_vf_key_2026_09_13`, test end-to-end.

---

## What ships as v1.0 (minimum)

- Tauri desktop app for macOS + Windows + Linux
- Signed installers with valid certs
- Indexer that walks local drives + external SSDs
- Basic browsing (Home, Events, Places, Videos, Library) — read-only for now
- Real-Stripe checkout → Ed25519 licence → machine binding
- 5-seat Family Pack + Personal single-seat
- Profile picker + PIN
- Support inbox + docs site + legal pages
- 24-step in-app tour (already built)
- Onboarding wizard (already built)
- Basic settings (profile, appearance, licence, about, danger)

**What v1.0 does NOT have:** face grouping, semantic search, upscaler, share links, socials imports beyond takeout parsing, cinematic intro, moodboards. Those are v1.1–v1.4.

That's a real, defensible, ship-able v1.0 that charges €89/149/249 (or €59/99/199 founding) and generates cash for the parity work.
