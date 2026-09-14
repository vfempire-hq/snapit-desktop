<div align="center">

# SnapIT — Desktop

**Own your photo library.**

Your photos live on your own storage. Your library format is portable and open.
On-device AI does search, faces, and restoration. The licence is one-off and
never expires. Nothing about your library ever leaves the device, and there is
no key material we could hand over even if compelled.

[![License: FSL-1.1-ALv2](https://img.shields.io/badge/license-FSL--1.1--ALv2-blue.svg)](LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app)
[![Version](https://img.shields.io/badge/version-0.1.7-brightgreen.svg)](https://github.com/vfempire-hq/snapit-desktop/releases)
[![security-scan](https://github.com/vfempire-hq/snapit-desktop/actions/workflows/security-scan.yml/badge.svg)](https://github.com/vfempire-hq/snapit-desktop/actions/workflows/security-scan.yml)

</div>

---

## Install

### Windows
```powershell
iex (iwr -useb https://snapit.vfempire.com/downloads/install.ps1).Content
```
Prebuilt NSIS installer, SHA-256 verified. No Rust, no VS Build Tools.

### Linux
```bash
curl -sSL https://snapit.vfempire.com/downloads/install.sh | bash
```
Signed `.deb` and `.AppImage`.

### macOS
Coming with the paid launch. Right-click → Open on first launch until we ship a
signed build.

### Build from source
```bash
git clone https://github.com/vfempire-hq/snapit-desktop
cd snapit-desktop/app
npm install
cargo tauri build
```

---

## What SnapIT actually does

- **Reads your existing folders.** No import ceremony. Point at a folder on your
  Mac, Windows PC, or a NAS mount and SnapIT indexes everything in place.
- **Never modifies the originals.** All edits are non-destructive and live in
  XMP sidecars + our SQLite catalog inside the same folder.
- **On-device AI.** Faces are clustered on your CPU/GPU. Semantic search
  (*"beach at sunset in italy"*) runs against embeddings computed here.
  Nothing about your library leaves the device unless you export it.
- **Portable format.** Copy the library folder to another machine. SnapIT there
  opens the same catalog and picks up where you left off. Your data outlives us.
- **Signed licence.** Ed25519-signed licence file, verified offline. Runs even
  if our purchase server disappears. Never phones home to check validity.

---

## What SnapIT deliberately does NOT do

- Ship its own cloud photo storage. That's the business model we exist to refuse.
- Auto-post to social networks.
- Sell prints, merch, or albums as an upsell.
- Recognise faces of people who did not consent — beyond the OS-level clustering
  that Apple / Google / Windows already do on your device.
- Send crash reports, analytics, or telemetry.
- Have any way for us to read your library.

---

## Pricing

One-off licence, no expiry, no re-purchase, no forced upgrades.

| Tier | Standard | Founding |
|---|---|---|
| **Personal** — one active install | €89 | €59 |
| **Family Pack** — up to five household installs | €149 | €99 |
| **Pro** — unlimited installs, RAW dev, plugin API | €249 | €199 |

Founding pricing is the intro run; standard prices go live once the launch
window closes. Every tier includes the Permanence Guarantee below.

---

## The Permanence Guarantee

**If VF Empire ever discontinues SnapIT**, within 90 days the source is opened
publicly, the sealed library format is documented in a public spec, the Ed25519
licence public key stays valid forever, and the last-shipped installer is
mirrored to a transparency log so anyone can independently reproduce a
byte-identical build. No online activation, no expiring licences, no "call
home" beacon exists in the code to break.

Full text: [snapit.vfempire.com/permanence](https://snapit.vfempire.com/permanence).

House-wide LAW as of 2026-09-13 — applies to every VF product from here on.

---

## Roadmap

| Phase | State | Scope |
|---|---|---|
| **R·01** | Shipped | Mock UI, on-device index, licence file, VF privacy kit, purchase flow. |
| **R·02a** | Shipped | Tauri desktop (Windows + Linux binaries), profile + PIN, catalog IPC, mock-to-native bridge, Stripe live. |
| **R·02b** | Next | macOS signed build, auto-updater, dedup, faces v2, RAW dev, NAS scan. |
| **R·03** | Planned | LAN peer sync, mobile companion, upscale + denoise, restore v2, slideshow / TV cast, printing marketplace. |

---

## Reproducible build

```bash
git clone https://github.com/vfempire-hq/snapit-desktop
cd snapit-desktop/app
npm install
cargo tauri build
```

Compare the SHA-256 of your installer to the one at
`snapit.vfempire.com/downloads/`. If they diverge — that's a security bug and
we owe you a public explanation.

---

## License

[FSL-1.1-ALv2](LICENSE) — the Functional Source License.

In plain terms:

- You can **read every line** of this code. Nothing about SnapIT is hidden.
- You can **run, modify, and share** it for personal use, evaluation,
  education, and research — free, forever.
- You **cannot build a competing photo-library product** from this code for
  the first two years after each version's release.
- Two years after each release, that version **automatically becomes
  Apache 2.0** — at which point you can do whatever you want with it.

Why FSL and not AGPL? Same trust story (source is public, verifiable, no
telemetry), plus a two-year window against direct commercial clones so we
can fund continuous development. It's the same license Sentry uses.

For commercial terms outside these bounds, contact `licensing@vfempire.com`.

## Trademark

**SnapIT** and **VF Empire** are trademarks of VF Empire Corp Ltd, Malta C 94160.
The code is FSL-1.1-ALv2; the name and logo are not — you can read the source
under the license terms but you can't ship a competing product under the
SnapIT brand.

---

<div align="center">
<sub>© 2026 VF Empire Corp Ltd · <a href="https://vfempire.com">vfempire.com</a> · Malta C 94160</sub>
</div>
