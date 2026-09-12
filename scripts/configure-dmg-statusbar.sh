#!/usr/bin/env bash
# Ensure a DMG Finder window shows the status bar when opened.
set -euo pipefail

DMG_PATH="${1:?DMG path required}"
VOLUME_NAME="${2:-Agent On Rails Setup}"

if [[ ! -f "$DMG_PATH" ]]; then
  echo "DMG not found: $DMG_PATH"
  exit 1
fi

if ! command -v osascript >/dev/null 2>&1; then
  echo "osascript not available — skipping DMG status bar configuration"
  exit 0
fi

DMG_PATH="$(cd "$(dirname "$DMG_PATH")" && pwd)/$(basename "$DMG_PATH")"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/aor-dmg.XXXXXX")"
WORK_DMG="$TMP_DIR/readwrite.dmg"
MOUNT_POINT=""

cleanup() {
  if [[ -n "$MOUNT_POINT" ]] && mount | grep -Fq "$MOUNT_POINT"; then
    hdiutil detach "$MOUNT_POINT" -quiet || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "==> Configuring DMG window status bar"
hdiutil convert "$DMG_PATH" -format UDRW -quiet -o "$WORK_DMG"

ATTACH_OUTPUT="$(hdiutil attach -readwrite -noverify -noautoopen "$WORK_DMG")"
MOUNT_POINT="$(echo "$ATTACH_OUTPUT" | awk '/\/Volumes\// {print $3; exit}')"

if [[ -z "$MOUNT_POINT" ]]; then
  echo "Warning: could not mount DMG for status bar configuration — continuing without it"
  exit 0
fi

if ! /usr/bin/osascript <<EOF
tell application "Finder"
  tell disk "$VOLUME_NAME"
    open
    delay 1
    tell container window
      set toolbar visible to false
      set statusbar visible to true
    end tell
    close
    open
    delay 1
    tell container window
      set statusbar visible to true
    end tell
    close
  end tell
end tell
EOF
then
  echo "Warning: Finder automation failed — continuing without status bar tweaks"
fi

hdiutil detach "$MOUNT_POINT" -quiet || true
MOUNT_POINT=""

FINAL_DMG="${DMG_PATH%.dmg}.configured.dmg"
hdiutil convert "$WORK_DMG" -format UDZO -quiet -o "$FINAL_DMG"
mv "$FINAL_DMG" "$DMG_PATH"

trap - EXIT
cleanup

echo "Configured status bar for: $DMG_PATH"
