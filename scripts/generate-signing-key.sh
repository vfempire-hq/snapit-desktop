#!/usr/bin/env bash
# Generate the Tauri updater signing keypair for SnapIT.
# Public half goes into app/src-tauri/tauri.conf.json under plugins.updater.pubkey.
# Private half stays OFFLINE (in ~/.snapit-signing/ on the dev machine, or in
# the sovereign vault on the SanDisk once M6 completes).

set -euo pipefail

DEST="${HOME}/.snapit-signing"
mkdir -p "$DEST"
if [[ -f "$DEST/snapit.key" ]]; then
    echo "Signing key already exists at $DEST/snapit.key — refusing to overwrite."
    echo "To rotate:"
    echo "  1. Move the old key aside: mv $DEST/snapit.key $DEST/snapit.key.bak-\$(date +%Y%m%d)"
    echo "  2. Re-run this script"
    echo "  3. Update app/src-tauri/tauri.conf.json → plugins.updater.pubkey with the new public key"
    exit 1
fi

if ! command -v cargo >/dev/null; then
    echo "cargo required — install rustup first" >&2
    exit 1
fi

cargo install tauri-cli --version '^2' --locked
cargo tauri signer generate -w "$DEST/snapit.key"

echo
echo "==> PRIVATE key (KEEP OFFLINE — never commit, never upload to CI as plaintext):"
echo "    $DEST/snapit.key"
echo
echo "==> PUBLIC key (paste into app/src-tauri/tauri.conf.json → plugins.updater.pubkey):"
echo
cat "$DEST/snapit.key.pub"
echo
echo "==> Set the following GH Actions secrets so CI can sign updates:"
echo "    TAURI_SIGNING_PRIVATE_KEY           (contents of $DEST/snapit.key)"
echo "    TAURI_SIGNING_PRIVATE_KEY_PASSWORD  (the passphrase you just entered)"
