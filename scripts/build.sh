#!/usr/bin/env bash
# SnapIT Desktop build helper.
# Run on Linux (dev VM or beast) or Mac. Windows uses cargo tauri build
# directly, or cross-compile from Linux via cargo-xwin.
#
#   ./scripts/build.sh              # native build for this host
#   ./scripts/build.sh --windows    # cross-compile Windows .exe via cargo-xwin
#   ./scripts/build.sh --deps-only  # install system deps and exit

set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"

DEPS_ONLY=0
WIN=0
for arg in "$@"; do
    case "$arg" in
        --deps-only) DEPS_ONLY=1 ;;
        --windows)   WIN=1 ;;
        *) echo "unknown flag: $arg" >&2; exit 1 ;;
    esac
done

# 1. system deps
if command -v apt-get >/dev/null; then
    echo "==> installing Tauri system deps (needs sudo)"
    sudo apt-get update
    sudo apt-get install -y \
        pkg-config libwebkit2gtk-4.1-dev \
        libgtk-3-dev libayatana-appindicator3-dev \
        librsvg2-dev libsoup-3.0-dev libssl-dev \
        build-essential curl wget file
fi

# 2. Rust + cargo-tauri
if ! command -v cargo >/dev/null; then
    echo "==> installing Rust via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
cargo install tauri-cli --version '^2' --locked

# 3. Windows-specific tooling (cross-compile via cargo-xwin)
if [[ $WIN -eq 1 ]]; then
    echo "==> installing cargo-xwin + clang + lld for cross-compile"
    if command -v apt-get >/dev/null; then
        sudo apt-get install -y clang lld llvm
    fi
    rustup target add x86_64-pc-windows-msvc
    cargo install cargo-xwin --locked
fi

if [[ $DEPS_ONLY -eq 1 ]]; then
    echo "==> deps installed, exiting"
    exit 0
fi

# 4. Frontend
if [[ ! -d app/node_modules ]]; then
    ( cd app && npm install )
fi
( cd app && npm run build )

# 5. Backend
if [[ $WIN -eq 1 ]]; then
    ( cd app/src-tauri && cargo xwin build --release --target x86_64-pc-windows-msvc )
    echo "==> Windows binary at app/src-tauri/target/x86_64-pc-windows-msvc/release/snapit.exe"
else
    ( cd app && npx tauri build 2>&1 | tail -8 )
fi
