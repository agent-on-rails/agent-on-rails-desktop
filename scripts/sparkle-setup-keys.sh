#!/usr/bin/env bash
# Generate Sparkle EdDSA keys (private in Keychain) and write public key for Info.plist.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PUBLIC_KEY_FILE="$ROOT_DIR/config/sparkle-public-ed-key.txt"
INFO_PLIST="$ROOT_DIR/src-tauri/Info.plist"
GENERATE_KEYS="$ROOT_DIR/src-tauri/sparkle-bin/generate_keys"

if [[ ! -x "$GENERATE_KEYS" ]]; then
  bash "$ROOT_DIR/scripts/download-sparkle.sh"
fi

echo "==> Generating Sparkle signing keys (login keychain)"
OUTPUT="$("$GENERATE_KEYS" 2>&1 || true)"
PUBLIC_KEY="$(echo "$OUTPUT" | sed -n 's/.*<string>\(.*\)<\/string>.*/\1/p' | head -1)"
if [[ -z "$PUBLIC_KEY" ]]; then
  # Already generated — export public key
  if [[ -x "$ROOT_DIR/src-tauri/sparkle-bin/generate_keys" ]]; then
    OUTPUT="$("$ROOT_DIR/src-tauri/sparkle-bin/generate_keys" -p 2>&1 || "$GENERATE_KEYS" 2>&1)"
    PUBLIC_KEY="$(echo "$OUTPUT" | sed -n 's/.*<string>\(.*\)<\/string>.*/\1/p' | head -1)"
  fi
fi
if [[ -z "$PUBLIC_KEY" ]]; then
  echo "$OUTPUT"
  echo "Could not parse Sparkle public key. If keys already exist, paste SUPublicEDKey into $PUBLIC_KEY_FILE"
  exit 1
fi

mkdir -p "$(dirname "$PUBLIC_KEY_FILE")"
printf '%s\n' "$PUBLIC_KEY" >"$PUBLIC_KEY_FILE"
echo "==> Saved public key → $PUBLIC_KEY_FILE"

if [[ -f "$INFO_PLIST" ]]; then
  /usr/libexec/PlistBuddy -c "Set :SUPublicEDKey $PUBLIC_KEY" "$INFO_PLIST" 2>/dev/null \
    || /usr/libexec/PlistBuddy -c "Add :SUPublicEDKey string $PUBLIC_KEY" "$INFO_PLIST"
  echo "==> Updated SUPublicEDKey in Info.plist"
fi

echo "Done. Keep the private key in Keychain; export SPARKLE_PRIVATE_KEY for CI if needed."
