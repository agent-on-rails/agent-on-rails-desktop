#!/usr/bin/env bash
# Download Sparkle.framework + generate_appcast / generate_keys tools into src-tauri/.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

curl -fsSL https://raw.githubusercontent.com/ahonn/tauri-plugin-sparkle-updater/refs/heads/master/scripts/download-sparkle.sh | bash

echo "Sparkle tools ready under src-tauri/sparkle-bin and Sparkle.framework"
