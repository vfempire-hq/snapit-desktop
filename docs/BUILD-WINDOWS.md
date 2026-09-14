# Building SnapIT for Windows

Two paths: **native** (build on a real Windows box or VM) or
**cross-compile from Linux** (via `cargo-xwin`, no Windows needed).

Cross-compile is the recommended path — no Windows licence needed,
runs on any Linux dev box, produces the same binary a real Windows
build would. Native is only worth doing if you're actually on Windows
anyway.

---

## Cross-compile from Linux (recommended)

### One-time setup

```bash
# 1. Rust targets
rustup target add x86_64-pc-windows-msvc

# 2. cargo-xwin — downloads MSVC libs on first build
cargo install cargo-xwin

# 3. clang + lld (linker) + llvm tools (archiver)
sudo apt install -y clang lld llvm

# 4. cargo-xwin expects these tools by MSVC-style names
sudo ln -sf /usr/bin/clang /usr/local/bin/clang-cl
sudo ln -sf /usr/bin/llvm-ar /usr/local/bin/llvm-lib

# 5. NSIS for the installer bundling step (optional; only for full installer)
sudo apt install -y nsis
```

### Why cargo-xwin and not mingw-w64?

The mingw-w64 toolchain fails when building the Tauri stack — the linker
hits a fixed limit on DLL symbol exports (`export ordinal too large:
88607`). Rust + Tauri + WebView2 + tokio + rusqlite exceed the ~65K
export cap in GNU ld. MSVC's lld-link has no such limit.

xwin downloads Microsoft's official MSVC redistributable libraries on
first run (~600MB cached to `~/.cache/cargo-xwin/`). Licensed under
Microsoft's redistributable terms — legal for CI use.

### Build

```bash
cd app/src-tauri
cargo xwin build --target x86_64-pc-windows-msvc --release --lib
```

Output binary: `target/x86_64-pc-windows-msvc/release/snapit.exe`

First build: ~15-20 min (downloads MSVC libs, compiles ring/webview2-com/
etc from C). Rebuilds: 30-60 seconds incremental.

### Full installer (.exe + .msi)

Tauri's `nsis` bundler runs on Linux; `wix` (for .msi) requires Windows.

```bash
cargo tauri build --target x86_64-pc-windows-msvc --bundles nsis
```

Output: `target/x86_64-pc-windows-msvc/release/bundle/nsis/
SnapIT_0.1.7_x64-setup.exe`

### Signing (Azure Trusted Signing)

Blocked as of 2026-09-13 — MS verification-code emails not arriving on
the VF Azure account. When unblocked:

```bash
# Env vars for tauri v2 signing
export TAURI_SIGNING_PRIVATE_KEY=/path/to/updater.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=…

# Sign the installer with Azure Trusted Signing (once Azure account works)
signtool sign /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 \
  /dlib /path/to/Azure.CodeSigning.Dlib.dll \
  /dmdf /path/to/metadata.json \
  target/x86_64-pc-windows-msvc/release/bundle/nsis/SnapIT_0.1.7_x64-setup.exe
```

Until then every install shows a SmartScreen "Windows protected your PC"
warning. Users can click "More info → Run anyway" to install. Not ideal
for retail but fine for beta + waitlist testers.

---

## Native (build on Windows itself)

If you're actually on Windows:

```powershell
# 1. Install Rust (rustup-init.exe from rustup.rs)

# 2. Install WebView2 runtime if not present
#    (Windows 11 has it; Windows 10 might not)
#    Download from: https://developer.microsoft.com/microsoft-edge/webview2/

# 3. Install Visual Studio Build Tools 2022 with "Desktop development with C++"
#    (Required for the MSVC linker)

# 4. Install Node.js (only if you plan to use `beforeDevCommand`)

# 5. Install tauri-cli
cargo install tauri-cli --version "^2.0"

# 6. Build
cd app/src-tauri
cargo tauri build --bundles nsis
```

Faster than cross-compile (no lib download), but requires a Windows box.

---

## Testing without signing

Install the unsigned installer:

```powershell
# Windows Defender will block by default. Right-click → Properties →
# Unblock, THEN run:
.\SnapIT_0.1.7_x64-setup.exe
```

Or bypass SmartScreen entirely by right-clicking → **Run as administrator**
(admin privileges skip the reputation check for this session).

## What ships in the .exe installer

- `snapit.exe` — the Tauri app
- WebView2 runtime loader (uses system-installed WebView2)
- Start menu shortcut + Desktop shortcut
- Uninstaller entry in Add/Remove Programs
- Registry entries for file-association (double-click a .jpg → Open with SnapIT)
- All the mock frontend embedded (feature wall, tour, settings, etc.)

## What DOESN'T ship in the .exe

- WebView2 runtime itself — the installer nags Windows to install it if
  missing (Windows 11 has it out of the box; some Windows 10 don't)
- Any user data — SnapIT stores everything in
  `%LOCALAPPDATA%\SnapIT\` at first launch

## Troubleshooting

**"MSVCP140.dll was not found"** — user needs Visual C++ Redistributable
2015-2022. Tauri can bundle it but it bloats the installer ~10 MB.
Add to tauri.conf.json:

```json
"bundle": {
  "windows": {
    "wix": { "language": "en-US" },
    "nsis": {
      "installMode": "perMachine",
      "installerIcon": "icons/icon.ico",
      "compression": "lzma"
    }
  }
}
```

**"Failed to find tool clang-cl"** — cargo-xwin needs the symlink from
the setup section. Re-run step 4.

**"llvm-lib: No such file or directory"** — needs `sudo apt install llvm`
then the symlink from step 4.

**"export ordinal too large"** — you're using mingw-w64, not MSVC.
Switch to `cargo xwin build --target x86_64-pc-windows-msvc`.
