#!/usr/bin/env bash
# SnapIT — one-command build-and-ship.
#
#   ./ship-update.sh              # bump patch version, build Linux + Windows,
#                                 # publish to snapit.vfempire.com/downloads/
#   VER=0.2.0 ./ship-update.sh    # ship a specific version
#   SKIP_WINDOWS=1 ./ship-update.sh  # Linux-only ship (fast iterate)
#   NO_GERMAN_PC=1 ./ship-update.sh  # don't trigger PC install after publish
#
# House-standard release lane (matches vfmail-desktop/ship-update.sh):
#   1. Bumps app/src-tauri/tauri.conf.json version.
#   2. Cross-compiles Windows .exe via cargo-xwin (from Linux, no PC required).
#   3. Builds Linux .deb + .AppImage.
#   4. Sha256-sums every artifact, copies into purchase-backend/public/downloads/.
#   5. Wrangler-deploys the purchase backend so snapit.vfempire.com/downloads/*
#      serves the new build immediately.
#   6. Optionally runs the freshly-shipped .exe on Vincent's German PC via the
#      desktop-agent (skip with NO_GERMAN_PC=1).
#
# Reads from .env.ship (git-ignored):
#   CLOUDFLARE_API_TOKEN
#   CLOUDFLARE_ACCOUNT_ID
#   AGENT_HOST=10.1.0.2:3989       (only needed when triggering PC install)
#   AGENT_TOKEN

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

CLOUDFLARE_API_TOKEN="${CLOUDFLARE_API_TOKEN:-}"
CLOUDFLARE_ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-}"
AGENT_HOST="${AGENT_HOST:-}"
AGENT_TOKEN="${AGENT_TOKEN:-}"

if [[ -f "$REPO_DIR/.env.ship" ]]; then
  # shellcheck disable=SC1091
  source "$REPO_DIR/.env.ship"
fi
: "${CLOUDFLARE_API_TOKEN:?CLOUDFLARE_API_TOKEN unset — put it in .env.ship or the environment}"
: "${CLOUDFLARE_ACCOUNT_ID:?CLOUDFLARE_ACCOUNT_ID unset — put it in .env.ship or the environment}"
export CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID

say()  { printf '\033[36m>> %s\033[0m\n' "$*"; }
ok()   { printf '\033[32m[ok] %s\033[0m\n' "$*"; }
warn() { printf '\033[33m[warn] %s\033[0m\n' "$*"; }
die()  { printf '\033[31m[fail] %s\033[0m\n' "$*" >&2; exit 1; }

command -v jq  >/dev/null || die "jq required"
command -v npx >/dev/null || die "npx required"
command -v cargo >/dev/null || die "cargo required"

# ---- 1. version ----
CONF="app/src-tauri/tauri.conf.json"
CUR_VER=$(jq -r '.version' "$CONF")
if [[ -n "${VER:-}" ]]; then
  NEW_VER="$VER"
else
  IFS='.' read -r MAJ MIN PATCH <<<"$CUR_VER"
  NEW_VER="${MAJ}.${MIN}.$((PATCH+1))"
fi
say "bumping ${CUR_VER} -> ${NEW_VER}"
tmp=$(mktemp); jq --arg v "$NEW_VER" '.version = $v' "$CONF" > "$tmp" && mv "$tmp" "$CONF"

# Cargo.toml holds a matching version in [package]
CARGO_TOML="app/src-tauri/Cargo.toml"
sed -i "s/^version = \"$CUR_VER\"/version = \"$NEW_VER\"/" "$CARGO_TOML"

# ---- 2. build Linux ----
say "building Linux .deb + .AppImage"
(
  cd app
  npm install --silent
  npx tauri build --bundles deb,appimage 2>&1 | tail -8
)
LINUX_DEB=$(find app/src-tauri/target/release/bundle/deb -name "*.deb" | head -1)
LINUX_APPIMG=$(find app/src-tauri/target/release/bundle/appimage -name "*.AppImage" | head -1)
[[ -f "$LINUX_DEB" ]]    || die ".deb not produced at expected path"
[[ -f "$LINUX_APPIMG" ]] || die ".AppImage not produced at expected path"
ok "Linux built"

# ---- 3. build Windows via cargo-xwin ----
if [[ -z "${SKIP_WINDOWS:-}" ]]; then
  say "cross-compiling Windows .exe via cargo-xwin (MSVC target)"
  command -v cargo-xwin >/dev/null || die "cargo-xwin required — install with: cargo install cargo-xwin"
  (
    cd app/src-tauri
    cargo xwin build --release --target x86_64-pc-windows-msvc 2>&1 | tail -6
  )
  # NSIS installer wrapping via tauri bundler on the Linux side isn't fully
  # supported yet — we ship the raw .exe from this run and expect the NSIS
  # bundle path to already exist from an earlier tauri-build. Keep the
  # existing .exe if present, otherwise warn.
  WIN_EXE=$(find app/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis -name "*x64-setup.exe" 2>/dev/null | head -1)
  if [[ -z "$WIN_EXE" ]]; then
    warn "no NSIS setup.exe — falling back to raw snapit.exe"
    WIN_EXE=$(find app/src-tauri/target/x86_64-pc-windows-msvc/release -maxdepth 1 -name "snapit.exe" | head -1)
    [[ -f "$WIN_EXE" ]] || die "no Windows binary at all"
  fi
  ok "Windows built: $(basename "$WIN_EXE")"
else
  warn "SKIP_WINDOWS=1 — Linux-only ship"
  WIN_EXE=""
fi

# ---- 4. publish to CDN ----
DL_DIR="purchase-backend/public/downloads"
mkdir -p "$DL_DIR"

publish() {
  local src="$1" name="$2"
  cp "$src" "$DL_DIR/$name"
  sha256sum "$DL_DIR/$name" | awk '{print $1}' > "$DL_DIR/$name.sha256"
  local size; size=$(stat -c%s "$DL_DIR/$name")
  ok "  $name ($(numfmt --to=iec "$size"))  sha=$(head -c 12 "$DL_DIR/$name.sha256")..."
}

say "publishing to $DL_DIR"
publish "$LINUX_DEB"     "SnapIT_${NEW_VER}_amd64.deb"
publish "$LINUX_DEB"     "snapit-latest_amd64.deb"
publish "$LINUX_APPIMG"  "SnapIT_${NEW_VER}_amd64.AppImage"
publish "$LINUX_APPIMG"  "snapit-latest_amd64.AppImage"
if [[ -n "$WIN_EXE" ]]; then
  publish "$WIN_EXE"     "SnapIT_${NEW_VER}_x64-setup.exe"
  publish "$WIN_EXE"     "snapit-latest_x64-setup.exe"
fi

# ---- 5. deploy purchase backend ----
say "deploying purchase-backend to Cloudflare"
(
  cd purchase-backend
  npx wrangler deploy 2>&1 | tail -4
)
ok "live at https://snapit.vfempire.com/downloads/"

# ---- 6. trigger German PC install (optional) ----
if [[ -z "${NO_GERMAN_PC:-}" ]] && [[ -n "$AGENT_HOST" ]] && [[ -n "$AGENT_TOKEN" ]] && [[ -n "$WIN_EXE" ]]; then
  say "kicking install on Vincent's PC"
  PS_SCRIPT="\$ErrorActionPreference='Continue'
\$dl = 'C:\\Users\\email\\SnapIT'
New-Item -ItemType Directory -Path \$dl -Force | Out-Null
Invoke-WebRequest -UseBasicParsing 'https://snapit.vfempire.com/downloads/snapit-latest_x64-setup.exe' -OutFile \"\$dl\\snapit-setup.exe\"
Get-Process snapit -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 700
Start-Process \"\$dl\\snapit-setup.exe\" -ArgumentList '/S' -Wait
Start-Sleep -Milliseconds 800
\$exe = Get-ChildItem 'C:\\Users\\email\\AppData\\Local\\SnapIT\\snapit.exe' -ErrorAction SilentlyContinue
if (\$exe) { Start-Process \$exe.FullName; Write-Host INSTALLED_AND_LAUNCHED } else { Write-Host INSTALL_FAILED }"
  enc=$(python3 -c "import sys,base64; print(base64.b64encode(sys.stdin.read().encode('utf-16-le')).decode())" <<<"$PS_SCRIPT")
  payload=$(python3 -c "import json,sys; print(json.dumps({'cmd':'powershell','args':['-NoProfile','-EncodedCommand',sys.argv[1]]}))" "$enc")
  R=$(curl -sS --max-time 120 -H "X-Token: $AGENT_TOKEN" -H "Content-Type: application/json" -d "$payload" "http://$AGENT_HOST/run" || true)
  if echo "$R" | grep -q INSTALLED_AND_LAUNCHED; then
    ok "installed and launched on PC"
  else
    warn "PC install did not confirm — response follows"
    echo "$R" | head -c 500; echo
  fi
fi

# ---- 7. commit version bump ----
if git diff --quiet "$CONF" "$CARGO_TOML"; then
  warn "no version diff to commit"
else
  git add "$CONF" "$CARGO_TOML"
  git commit -m "release: v${NEW_VER}" >/dev/null
  ok "committed release: v${NEW_VER}"
fi

ok "SnapIT v${NEW_VER} shipped"
