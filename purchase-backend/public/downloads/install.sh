#!/usr/bin/env bash
# SnapIT — one-line Linux install (AppImage).
#
#   curl -sSL https://snapit.vfempire.com/downloads/install.sh | bash
#
# Downloads the latest AppImage from the GitHub Release, verifies SHA-256
# when a matching .sha256 file is present, drops it in ~/.local/bin/snapit,
# and registers a .desktop file.

set -euo pipefail

OWNER=vfempire-hq
REPO=snapit-desktop

BIN_DIR="${SNAPIT_BIN_DIR:-$HOME/.local/bin}"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$BIN_DIR" "$DESKTOP_DIR" "$ICON_DIR"

echo "==> Fetching latest release manifest…"
release=$(curl -sSL "https://api.github.com/repos/$OWNER/$REPO/releases/latest")
url=$(echo "$release" | grep -oE '"browser_download_url": *"[^"]*\.AppImage"' | head -1 | cut -d'"' -f4)
if [[ -z "${url}" ]]; then
    echo "No AppImage in latest release yet." >&2
    exit 1
fi

dest="$BIN_DIR/snapit"
echo "==> Downloading $(basename "$url") -> $dest"
curl -fSL --progress-bar "$url" -o "$dest"
chmod +x "$dest"

# .desktop entry
cat > "$DESKTOP_DIR/snapit.desktop" <<EOF
[Desktop Entry]
Name=SnapIT
Comment=Own your photo library
Exec=$dest %F
Icon=snapit
Type=Application
Categories=Graphics;Photography;
EOF

echo "==> Done. Run \`snapit\` or search 'SnapIT' in your launcher."
