# Building SnapIT for macOS

macOS .dmg installers must be built on a Mac. Apple's build tools
(`iconutil`, `productbuild`, `hdiutil`, `codesign`, `notarytool`) are
macOS-only and Apple restricts distribution of the macOS SDK.

Everything else in the pipeline is portable — the Rust code, the mock
frontend, the Tauri config, the icons. This document is what you run
on a Mac to produce a signed, notarised `.dmg`.

---

## One-time setup (Mac)

Apple silicon (M1/M2/M3/M4) preferred — it can build both Universal
and arm64/x86_64. Intel Mac can also build both via Rosetta.

```bash
# 1. Xcode Command Line Tools
xcode-select --install

# 2. Homebrew if not already
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 3. Rust + Tauri CLI
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo install tauri-cli --version "^2.0"

# 4. Clone the repo
git clone https://github.com/vfempire/snapit-desktop.git
cd snapit-desktop
```

## Build

```bash
cd app/src-tauri

# Both architectures at once (Universal)
cargo tauri build --target universal-apple-darwin --bundles dmg,app

# Or per-arch (~40% smaller each)
cargo tauri build --target aarch64-apple-darwin --bundles dmg,app  # Apple silicon
cargo tauri build --target x86_64-apple-darwin --bundles dmg,app   # Intel
```

Output lands at:
```
target/{arch}/release/bundle/dmg/SnapIT_0.1.7_{arch}.dmg
target/{arch}/release/bundle/macos/SnapIT.app
```

## Signing

Apple Developer ID required. Load the cert into Keychain first, then:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: VF Empire Corp Ltd (TEAMID)"
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="app-specific-password-from-appleid.apple.com"
export APPLE_TEAM_ID="TEAMID"

# Rebuild with signing on
cargo tauri build --target universal-apple-darwin --bundles dmg,app
```

Tauri v2 will sign the .app and .dmg automatically.

## Notarisation (required for macOS 10.15+)

Apple's Gatekeeper blocks unnotarised apps by default. Tauri v2 does
this in the build step when the env vars above are set. Manual command:

```bash
xcrun notarytool submit \
  target/universal-apple-darwin/release/bundle/dmg/SnapIT_0.1.7_universal.dmg \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

# Once notarised, staple the ticket so offline installs work
xcrun stapler staple target/universal-apple-darwin/release/bundle/dmg/SnapIT_0.1.7_universal.dmg
```

Notarisation takes 2-15 minutes.

## Verify

```bash
# Codesign valid?
codesign --verify --deep --strict --verbose=2 \
  target/universal-apple-darwin/release/bundle/macos/SnapIT.app

# Notarisation stapled?
xcrun stapler validate \
  target/universal-apple-darwin/release/bundle/dmg/SnapIT_0.1.7_universal.dmg

# Gatekeeper approves?
spctl --assess --type execute --verbose \
  target/universal-apple-darwin/release/bundle/macos/SnapIT.app
```

All three should report success. Ship it.

## What blocks this today (2026-09-14)

Per memory `project_azure_signing_blocked_2026_09_13`:
- Azure Trusted Signing (Windows) — MS verification-code emails
  not arriving, blocking all VF Windows signing including SnapIT.

For macOS specifically, blocker is task #40:
- Apple Developer API key access verification is unresolved.
  Without it, `notarytool submit` will fail with `invalid credentials`.

Unblock these before public v1.0.

## Alternatives to running Mac locally

- **GitHub Actions macOS runner**: cost-free for public repos, or
  ~$0.16/minute private. Set up secrets for signing + notarisation,
  push tag, artefact appears on Releases page. Setup: 30 min.
- **MacStadium / AWS EC2 mac1.metal**: $1-2/hour, SSH-accessible.
  Use for one-off builds when you don't own a Mac.
- **iCloud-connected Mac at a friend's**: fastest for solo dev if
  you just need to prove a build works.

## Universal .dmg — what does it mean?

A Universal binary contains **both x86_64 and arm64 code**. The .dmg
is ~2× the size of a single-arch build (~15 MB vs ~8 MB for SnapIT),
but it runs natively on any Mac made since 2009 with no Rosetta.

Recommended for public v1.0 — a single download link that Just Works
on every Mac, no "which chip do I have?" questions.
