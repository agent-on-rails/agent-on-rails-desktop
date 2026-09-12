#!/usr/bin/env bash
# Drag-and-drop DMG: app on the left, Applications drop-link on the right (Nucleus layout).
# Finder AppleScript needs a GUI session — do not run this under CI=true.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:?Usage: create-dmg.sh <App.app> [output.dmg] [volume name]}"
OUTPUT_DMG="${2:-$ROOT_DIR/dist-release/Agent-On-Rails-macos.dmg}"
VOLUME_NAME="${3:-Agent On Rails}"
BACKGROUND="$ROOT_DIR/scripts/dmg-background.png"

# create-dmg skips Finder layout when Jenkins/CI env is set.
unset CI JENKINS_HOME BUILD_NUMBER TEAMCITY_VERSION || true

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

if [[ ! -f "$BACKGROUND" ]]; then
  echo "==> Generating DMG background"
  swift "$ROOT_DIR/scripts/generate-dmg-background.swift" "$BACKGROUND"
fi

mkdir -p "$(dirname "$OUTPUT_DMG")"
rm -f "$OUTPUT_DMG"

STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/aor-dmg-stage.XXXXXX")"
cleanup() {
  rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

# Stage a folder so the DMG contains the .app bundle (not its Contents/).
cp -R "$APP_PATH" "$STAGING_DIR/$APP_NAME"

if command -v create-dmg >/dev/null 2>&1; then
  echo "==> Creating drag-and-drop DMG with create-dmg"
  CREATE_DMG_ARGS=(
    --volname "$VOLUME_NAME"
    --background "$BACKGROUND"
    --window-pos 200 120
    --window-size 660 400
    --icon-size 128
    --text-size 14
    --icon "$APP_NAME" 180 185
    --hide-extension "$APP_NAME"
    --app-drop-link 480 185
    --no-internet-enable
  )
  if [[ -n "$ICON_PATH" ]]; then
    CREATE_DMG_ARGS=(--volicon "$ICON_PATH" "${CREATE_DMG_ARGS[@]}")
  fi
  set +e
  create-dmg "${CREATE_DMG_ARGS[@]}" "$OUTPUT_DMG" "$STAGING_DIR"
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
