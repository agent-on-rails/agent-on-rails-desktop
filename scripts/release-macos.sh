#!/usr/bin/env bash
# Signed macOS release: build → notarize → Sparkle appcast → copy to marketing site downloads/.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
# shellcheck disable=SC1091
source "$ROOT_DIR/scripts/release-env.sh"

VERSION="$(node -p "require('./package.json').version")"
OUT_DIR="$ROOT_DIR/dist-release"
WEBSITE_DOWNLOADS="${AOR_WEBSITE_DOWNLOADS:-$ROOT_DIR/../agent-on-rails-website/public/downloads}"
SPARKLE_FRAMEWORK="$ROOT_DIR/src-tauri/Sparkle.framework"
GENERATE_APPCAST="$ROOT_DIR/src-tauri/sparkle-bin/generate_appcast"

if [[ ! -d "$SPARKLE_FRAMEWORK" || ! -x "$GENERATE_APPCAST" ]]; then
  bash "$ROOT_DIR/scripts/download-sparkle.sh"
fi

if [[ ! -f "$ROOT_DIR/config/sparkle-public-ed-key.txt" ]] \
  || grep -q REPLACE_WITH_SPARKLE "$ROOT_DIR/src-tauri/Info.plist"; then
  bash "$ROOT_DIR/scripts/sparkle-setup-keys.sh"
fi

# Ensure Tauri updater keys exist (Windows + shared latest.json signing)
if [[ ! -f "$ROOT_DIR/config/tauri-updater-pubkey.txt" ]]; then
  echo "==> Generating Tauri updater keypair"
  mkdir -p "$ROOT_DIR/config"
  npx --yes @tauri-apps/cli signer generate -w "$ROOT_DIR/config/tauri-updater.key" -p "" >/tmp/aor-tauri-signer.txt 2>&1 || true
  if [[ -f "$ROOT_DIR/config/tauri-updater.key.pub" ]]; then
    cp "$ROOT_DIR/config/tauri-updater.key.pub" "$ROOT_DIR/config/tauri-updater-pubkey.txt"
  else
    # Fallback: parse from CLI output file if present
    PUB="$(rg -o 'dW50[A-Za-z0-9+/=]+' /tmp/aor-tauri-signer.txt | head -1 || true)"
    if [[ -n "$PUB" ]]; then
      printf '%s\n' "$PUB" >"$ROOT_DIR/config/tauri-updater-pubkey.txt"
    fi
  fi
fi

PUBKEY="$(tr -d '\n' <"$ROOT_DIR/config/tauri-updater-pubkey.txt" 2>/dev/null || true)"
if [[ -n "$PUBKEY" ]]; then
  python3 - <<PY
import json, pathlib
p = pathlib.Path("$ROOT_DIR/src-tauri/tauri.conf.json")
cfg = json.loads(p.read_text())
plugins = cfg.setdefault("plugins", {})
updater = plugins.setdefault("updater", {})
updater["pubkey"] = """$PUBKEY""".strip()
updater["endpoints"] = ["https://agent-on-rails.suherman.net/downloads/latest.json"]
updater.setdefault("windows", {})["installMode"] = "passive"
bundle = cfg.setdefault("bundle", {})
bundle["createUpdaterArtifacts"] = False  # Sparkle appcast on macOS; Windows CI signs updater artifacts
mac = bundle.setdefault("macOS", {})
mac["signingIdentity"] = """$APPLE_SIGNING_IDENTITY"""
mac["frameworks"] = ["Sparkle.framework"]
p.write_text(json.dumps(cfg, indent=2) + "\n")
print("Updated tauri.conf.json pubkey + signingIdentity")
PY
fi

sign_bundle_for_notarization() {
  local app="$1"
  local identity="${APPLE_SIGNING_IDENTITY}"
  local sparkle="$app/Contents/Frameworks/Sparkle.framework"
  local flags=(--force --options runtime --timestamp --sign "$identity")
  if [[ -d "$sparkle" ]]; then
    echo "==> Signing Sparkle nested binaries"
    if [[ -x "$sparkle/Versions/B/Autoupdate" ]]; then
      codesign "${flags[@]}" "$sparkle/Versions/B/Autoupdate"
    fi
    if [[ -d "$sparkle/Versions/B/Updater.app" ]]; then
      codesign "${flags[@]}" "$sparkle/Versions/B/Updater.app/Contents/MacOS/Updater"
      codesign "${flags[@]}" "$sparkle/Versions/B/Updater.app"
    fi
    for xpc in "$sparkle/Versions/B/XPCServices"/*.xpc; do
      [[ -d "$xpc" ]] || continue
      codesign "${flags[@]}" "$xpc"
    done
    codesign "${flags[@]}" "$sparkle"
  fi
  echo "==> Re-signing $(basename "$app")"
  codesign "${flags[@]}" "$app"
  codesign --verify --deep --strict "$app"
}

mkdir -p "$OUT_DIR"
# Cursor/CI may point cargo at a sandbox cache; keep release artifacts in-tree
# but still discover bundles written to that cache.
SAVED_CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-}"
unset CARGO_TARGET_DIR
export CI=true
export APPLE_SIGNING_IDENTITY
# Plugin build.rs looks for Sparkle next to OUT_DIR ancestors; CARGO_TARGET_DIR may break that.
export SPARKLE_FRAMEWORK_PATH="${SPARKLE_FRAMEWORK_PATH:-$ROOT_DIR/src-tauri}"
export TAURI_SIGNING_PRIVATE_KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$ROOT_DIR/config/tauri-updater.key}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

echo "==> Building signed macOS bundles (v${VERSION})"
if [[ "${SKIP_BUILD:-0}" == "1" ]]; then
  echo "    SKIP_BUILD=1 — using existing $OUT_DIR bundle"
else
  npm install
  npm run tauri build -- --bundles app

  find_bundle() {
    local kind="$1"
    local glob="$2"
    local p
    for dir in \
      "$ROOT_DIR/src-tauri/target/release/bundle/$kind" \
      "${SAVED_CARGO_TARGET_DIR}/release/bundle/$kind"
    do
      [[ -d "$dir" ]] || continue
      p="$(find "$dir" -maxdepth 1 -name "$glob" -print0 2>/dev/null | xargs -0 ls -td 2>/dev/null | head -1 || true)"
      if [[ -n "$p" && ( -d "$p" || -f "$p" ) ]]; then
        echo "$p"
        return 0
      fi
    done
  }

  APP_PATH="$(find_bundle macos '*.app')"

  if [[ ! -d "$APP_PATH" ]]; then
    echo "error: .app not found after build"
    exit 1
  fi

  APP_NAME="$(basename "$APP_PATH")"
  rm -rf "$OUT_DIR/$APP_NAME"
  cp -R "$APP_PATH" "$OUT_DIR/"
fi

APP_NAME="${APP_NAME:-Agent On Rails Setup.app}"
if [[ ! -d "$OUT_DIR/$APP_NAME" ]]; then
  echo "error: $OUT_DIR/$APP_NAME not found"
  exit 1
fi

sign_bundle_for_notarization "$OUT_DIR/$APP_NAME"

if should_notarize; then
  append_notary_auth_args
  echo "==> Notarizing $APP_NAME"
  ZIP_NOTARY="$(mktemp -t aor-notarize.XXXXXX.zip)"
  COPYFILE_DISABLE=1 ditto -c -k --sequesterRsrc --keepParent "$OUT_DIR/$APP_NAME" "$ZIP_NOTARY"
  SUBMIT_JSON="$(mktemp)"
  xcrun notarytool submit "$ZIP_NOTARY" "${NOTARY_AUTH_ARGS[@]}" --output-format json >"$SUBMIT_JSON"
  SUBMISSION_ID="$(python3 -c "import json; print(json.load(open('$SUBMIT_JSON')).get('id',''))")"
  rm -f "$SUBMIT_JSON" "$ZIP_NOTARY"
  if [[ -z "$SUBMISSION_ID" ]]; then
    echo "error: notarization submit failed"
    exit 1
  fi
  echo "    Submission: $SUBMISSION_ID"
  xcrun notarytool wait "$SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" --timeout 1h --progress
  STATUS="$(xcrun notarytool info "$SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" --output-format json | python3 -c "import json,sys; print(json.load(sys.stdin).get('status',''))")"
  if [[ "$STATUS" != "Accepted" ]]; then
    echo "Notarization status: $STATUS"
    xcrun notarytool log "$SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" || true
    exit 1
  fi
  xcrun stapler staple "$OUT_DIR/$APP_NAME"
  echo "==> Creating drag-and-drop DMG"
  DMG_OUT="$OUT_DIR/Agent-On-Rails-Setup-${VERSION}-macos.dmg"
  rm -f "$DMG_OUT"
  bash "$ROOT_DIR/scripts/create-dmg.sh" "$OUT_DIR/$APP_NAME" "$DMG_OUT" "Agent On Rails Setup"
  echo "==> Signing DMG"
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$DMG_OUT"
  echo "==> Notarizing DMG"
  SUBMIT_JSON="$(mktemp)"
  xcrun notarytool submit "$DMG_OUT" "${NOTARY_AUTH_ARGS[@]}" --output-format json >"$SUBMIT_JSON"
  DMG_SUBMISSION_ID="$(python3 -c "import json; print(json.load(open('$SUBMIT_JSON')).get('id',''))")"
  rm -f "$SUBMIT_JSON"
  if [[ -z "$DMG_SUBMISSION_ID" ]]; then
    echo "error: DMG notarization submit failed"
    exit 1
  fi
  echo "    DMG submission: $DMG_SUBMISSION_ID"
  xcrun notarytool wait "$DMG_SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" --timeout 1h --progress
  DMG_STATUS="$(xcrun notarytool info "$DMG_SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" --output-format json | python3 -c "import json,sys; print(json.load(sys.stdin).get('status',''))")"
  if [[ "$DMG_STATUS" != "Accepted" ]]; then
    echo "DMG notarization status: $DMG_STATUS"
    xcrun notarytool log "$DMG_SUBMISSION_ID" "${NOTARY_AUTH_ARGS[@]}" || true
    exit 1
  fi
  xcrun stapler staple "$DMG_OUT"
  xcrun stapler validate "$DMG_OUT"
else
  echo "warning: notarization skipped (configure AC_NOTARY or APPLE_APP_SPECIFIC_PASSWORD)"
  DMG_OUT="$OUT_DIR/Agent-On-Rails-Setup-${VERSION}-macos.dmg"
  bash "$ROOT_DIR/scripts/create-dmg.sh" "$OUT_DIR/$APP_NAME" "$DMG_OUT" "Agent On Rails Setup"
fi

# Sparkle ZIP + appcast
SPARKLE_DIR="$OUT_DIR/sparkle"
mkdir -p "$SPARKLE_DIR"
# generate_appcast rejects two zips with the same bundle version.
find "$SPARKLE_DIR" -maxdepth 1 -name '*.zip' -delete
ZIP_PATH="$SPARKLE_DIR/Agent-On-Rails-Setup-${VERSION}.zip"
COPYFILE_DISABLE=1 ditto -c -k --sequesterRsrc --keepParent "$OUT_DIR/$APP_NAME" "$ZIP_PATH"

echo "==> Generating Sparkle appcast"
"$GENERATE_APPCAST" \
  --download-url-prefix "${AOR_DOWNLOAD_BASE}/" \
  --maximum-deltas 0 \
  -o "$SPARKLE_DIR/appcast.xml" \
  "$SPARKLE_DIR"

# Publish into marketing site public/downloads
mkdir -p "$WEBSITE_DOWNLOADS"
cp -f "$ZIP_PATH" "$WEBSITE_DOWNLOADS/Agent-On-Rails-Setup-${VERSION}.zip"
cp -f "$ZIP_PATH" "$WEBSITE_DOWNLOADS/Agent-On-Rails-Setup-macos.zip"
cp -f "$SPARKLE_DIR/appcast.xml" "$WEBSITE_DOWNLOADS/appcast.xml"
if [[ -f "$OUT_DIR/Agent-On-Rails-Setup-${VERSION}-macos.dmg" ]]; then
  cp -f "$OUT_DIR/Agent-On-Rails-Setup-${VERSION}-macos.dmg" "$WEBSITE_DOWNLOADS/"
  cp -f "$OUT_DIR/Agent-On-Rails-Setup-${VERSION}-macos.dmg" "$WEBSITE_DOWNLOADS/Agent-On-Rails-Setup-macos.dmg"
fi

# Seed latest.json (macOS entry; Windows CI merges later)
python3 - <<PY
import json, pathlib, datetime
from pathlib import Path
downloads = Path("$WEBSITE_DOWNLOADS")
version = "$VERSION"
zip_name = f"Agent-On-Rails-Setup-{version}.zip"
# Prefer existing latest.json merge
path = downloads / "latest.json"
data = {"version": version, "notes": "Agent On Rails Setup", "pub_date": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": {}}
if path.exists():
    try:
        data = json.loads(path.read_text())
        data["version"] = version
    except Exception:
        pass
# Tauri updater platform keys: darwin-aarch64 / darwin-x86_64
import platform
machine = platform.machine().lower()
arch = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
url = f"https://agent-on-rails.suherman.net/downloads/{zip_name}"
# Signature file from tauri build if present
sig = ""
sig_candidates = list(Path("$ROOT_DIR/src-tauri/target/release/bundle").rglob("*.sig"))
if sig_candidates:
    sig = sig_candidates[0].read_text().strip()
data.setdefault("platforms", {})[f"darwin-{arch}"] = {"url": url, "signature": sig}
path.write_text(json.dumps(data, indent=2) + "\n")
print("Wrote", path)
PY

echo ""
echo "macOS release ready:"
echo "  $OUT_DIR"
echo "  $WEBSITE_DOWNLOADS"
echo "Next: commit website public/downloads + deploy marketing site."
