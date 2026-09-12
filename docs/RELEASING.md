# Agent On Rails — signed releases

## Platforms

| OS | Installer | Auto-update | Signing |
| --- | --- | --- | --- |
| macOS | `.dmg` + Sparkle `.zip` | **Sparkle** (`appcast.xml`) | Developer ID Application (Huge Shop) + notarization |
| Windows | `.msi` / NSIS | Tauri updater (`latest.json`) | Authenticode via `WINDOWS_CERTIFICATE` CI secret |

Sparkle is macOS-only. Windows uses the official Tauri updater with the same download CDN on the marketing site.

## One-time setup (macOS)

```bash
# Sparkle framework + tools
bash scripts/download-sparkle.sh
bash scripts/sparkle-setup-keys.sh

# Optional local overrides
cp .env.release.example .env.release
```

Notarization uses Keychain profile `AC_NOTARY` (same as Nucleus / Video Hub) or
`APPLE_ID` + `APPLE_APP_SPECIFIC_PASSWORD` + `APPLE_TEAM_ID`.

## Build & publish macOS

```bash
bash scripts/release-macos.sh
# Artifacts → dist-release/ and copied into ../agent-on-rails-website/public/downloads/
```

Then deploy the website (image + Argo) so
https://agent-on-rails.suherman.net/downloads/appcast.xml is live.

macOS DMG is a **drag-and-drop installer** (app + Applications alias + arrow background), matching Nucleus. Open the DMG and drag **Agent On Rails** into Applications. Sparkle **Check for Updates…** is in the app menu and the wizard header.

`create-dmg` (Homebrew) produces the Finder layout; without it the script falls back to `hdiutil` plus an Applications symlink.

## Windows (CI)

Push a tag `desktop-v*` or run the **Release** workflow. Secrets:

- `WINDOWS_CERTIFICATE` — base64 PFX
- `WINDOWS_CERTIFICATE_PASSWORD`
- `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — updater signatures
- `TAURI_UPDATER_PUBKEY` — already embedded in `tauri.conf.json`

### Important

Until `WINDOWS_CERTIFICATE` (and `WINDOWS_CERTIFICATE_PASSWORD`) are set on the GitHub repo, do **not** publish an unsigned MSI as the primary website download. macOS remains the signed primary release; Windows can stay on GitHub Releases once a signed build exists.

## Feed URLs

- Sparkle: `https://agent-on-rails.suherman.net/downloads/appcast.xml`
- Tauri updater: `https://agent-on-rails.suherman.net/downloads/latest.json`
