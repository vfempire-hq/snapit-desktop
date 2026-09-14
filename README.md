<div align="center">

# 📷  SnapIT

**Own your photo library.**

SnapIT is a photo library manager where your photos live on your own storage,
your library format is portable and open, on-device AI does search and faces
and restoration, and the licence is one-off — €69 for personal, €129 for family
and pro. It never expires. It never phones home. It never trains on your library.

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app)
[![Status](https://img.shields.io/badge/status-R%C2%B701_IN_BUILD-yellow.svg)]()
[![security-scan](https://github.com/vfempire-hq/snapit-desktop/actions/workflows/security-scan.yml/badge.svg)](https://github.com/vfempire-hq/snapit-desktop/actions/workflows/security-scan.yml)

</div>

---

## What SnapIT actually does

- **Reads your existing folders.** No import ceremony. Point at a folder on your
  Mac, Windows PC, or a NAS mount and SnapIT indexes everything in place.
- **Never modifies the originals.** All edits are non-destructive and live in
  XMP sidecars + our SQLite catalog inside the same folder.
- **On-device AI.** Faces are clustered on your CPU/GPU. Semantic search
  ("beach at sunset in italy") runs against embeddings computed here.
  Nothing about your library leaves the device unless you export it.
- **Portable format.** Copy the library folder to another machine. SnapIT there
  opens the same catalog and picks up where you left off. Your data outlives us.

## Install

### Windows
```powershell
iex(iwr -useb https://snapit.vfempire.com/downloads/install.ps1).Content
```

### macOS
Download the `.dmg` from [snapit.vfempire.com](https://snapit.vfempire.com) —
right-click → Open on first launch until we get a signed build.

### Linux
```bash
curl -sSL https://snapit.vfempire.com/downloads/install.sh | bash
```

## What it does NOT do

- Ship its own cloud photo storage. That's the model we exist to refuse.
- Auto-post to social networks.
- Sell prints, merch, or albums as an upsell.
- Recognise faces of people who did not consent — beyond the OS-level clustering
  that Apple/Google/Windows already do on your device.
- Send crash reports, analytics, or telemetry.

## The Permanence Guarantee

**If VF Empire ever discontinues SnapIT**, within 90 days the source is opened publicly, the sealed library format is documented in a public spec, the Ed25519 licence public key stays valid forever, and the last-shipped installer is mirrored to a transparency log so anyone can independently reproduce a byte-identical build. No online activation, no expiring licences, no "call home" beacon exists in the code to break.

Full text: [snapit.vfempire.com/permanence](https://snapit.vfempire.com/permanence).

House-wide LAW as of 2026-09-13 — applies to every VF product from here on.

## Roadmap (public)

| Phase | State | Scope |
|---|---|---|
| **R·01** | **IN BUILD** | Windows + macOS, local + NAS, import + edit stack + faces + search + purchase |
| **R·02** | Planned | Linux, S3-compat storage, RAW dev, upscale + denoise, LAN peer sync, mobile companion |
| **R·03** | Planned | Advanced restore, slideshow / TV cast, panorama & HDR merge, printing marketplace |

## Reproducible build

```
git clone https://github.com/vfempire-hq/snapit-desktop
cd snapit-desktop/app
npm install
cargo tauri build
```

Compare the SHA-256 of your installer to the one at
`snapit.vfempire.com/downloads/`. If they diverge — that's a security bug and
we owe you a public explanation.

## License

[AGPL-3.0](LICENSE). If you fork SnapIT and run it as a hosted service, you
must share the source of your modified version with your users. If you want
commercial terms without the copyleft, contact `licensing@vfempire.com`.

## Trademark

**SnapIT** and **VF Empire** are trademarks of VF Empire Corp Ltd, Malta C 94160.
The code is AGPL; the name and logo are not — you can fork but not ship a
competing product under the SnapIT brand.

---

<div align="center">
<sub>© 2026 VF Empire Corp Ltd · <a href="https://vfempire.com">vfempire.com</a> · Malta C 94160</sub>
</div>
