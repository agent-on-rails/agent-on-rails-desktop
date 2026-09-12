#!/usr/bin/env bash
# Drag-and-drop DMG: app on the left, Applications drop-link on the right (Nucleus layout).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:?Usage: create-dmg.sh <App.app> [output.dmg] [volume name]}"
OUTPUT_DMG="${2:-$ROOT_DIR/dist-release/Agent-On-Rails-Setup-macos.dmg}"
VOLUME_NAME="${3:-Agent On Rails Setup}"

if [[ ! -d "$APP_PATH" ]]; then
  echo "App not found: $APP_PATH"
  exit 1
fi

APP_NAME="$(basename "$APP_PATH")"
ICON_CANDIDATES=(
  "$APP_PATH/Contents/Resources/icon.icns"
  "$APP_PATH/Contents/Resources/AppIcon.icns"
  "$ROOT_DIR/src-tauri/icons/icon.icns"
)

ICON_PATH=""
for candidate in "${ICON_CANDIDATES[@]}"; do
  if [[ -f "$candidate" ]]; then
    ICON_PATH="$candidate"
    break
  fi
done

mkdir -p "$(dirname "$OUTPUT_DMG")"
rm -f "$OUTPUT_DMG"

if command -v create-dmg >/dev/null 2>&1; then
  echo "==> Creating drag-and-drop DMG with create-dmg"
  CREATE_DMG_ARGS=(
    --volname "$VOLUME_NAME"
    --window-pos 200 120
    --window-size 660 400
    --icon-size 128
    --icon "$APP_NAME" 180 185
    --hide-extension "$APP_NAME"
    --app-drop-link 480 185
  )
  if [[ -n "$ICON_PATH" ]]; then
    CREATE_DMG_ARGS=(--volicon "$ICON_PATH" "${CREATE_DMG_ARGS[@]}")
  fi
  # create-dmg sometimes exits 2 when Finder layout tweaks fail but still writes the DMG.
  set +e
  create-dmg "${CREATE_DMG_ARGS[@]}" "$OUTPUT_DMG" "$APP_PATH"
  CREATE_STATUS=$?
  set -e
  if [[ ! -f "$OUTPUT_DMG" ]]; then
    echo "create-dmg failed (exit $CREATE_STATUS) and produced no DMG"
    exit 1
  fi
  bash "$ROOT_DIR/scripts/configure-dmg-statusbar.sh" "$OUTPUT_DMG" "$VOLUME_NAME" || {
    echo "Warning: DMG status bar configuration skipped"
  }
else
  echo "==> Creating DMG with hdiutil (install create-dmg for the Nucleus layout: brew install create-dmg)"
  STAGING_DIR="$(mktemp -d)"
  trap 'rm -rf "$STAGING_DIR"' EXIT
  cp -R "$APP_PATH" "$STAGING_DIR/$APP_NAME"
  ln -s /Applications "$STAGING_DIR/Applications"
  hdiutil create \
    -volname "$VOLUME_NAME" \
    -srcfolder "$STAGING_DIR" \
    -ov \
    -format UDZO \
    "$OUTPUT_DMG" >/dev/null
  bash "$ROOT_DIR/scripts/configure-dmg-statusbar.sh" "$OUTPUT_DMG" "$VOLUME_NAME" || {
    echo "Warning: DMG status bar configuration skipped"
  }
fi

echo "Created: $OUTPUT_DMG"
