# Agent On Rails — Desktop Setup (macOS / Windows / Linux)

Thin **setup wizard** for first-time operators (control-plane [AOR-008](https://github.com/agent-on-rails/agent-on-rails-control-plane/blob/main/specs/AOR-008-desktop-setup/spec.md), [ADR-008](https://github.com/agent-on-rails/agent-on-rails-control-plane/blob/main/adr/ADR-008-desktop-setup-wizard.md)).

It does **not** replace the `aor` CLI/TUI. Flow:

1. Check Python 3.11+ / pipx / `aor`
2. Install or upgrade the CLI from `agent-on-rails-cli`
3. Run `aor init` for a docs+specs project
4. Optional: paste requirements → confirm outline → write SurveyDesk-shaped `specs/` (`aor gather`, AOR-010)
5. Hand off to [walkthrough](https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/walkthrough.md)

Stack: **Tauri 2** + TypeScript (vanilla).

## Downloads

| Platform | How |
| --- | --- |
| **macOS** | Signed DMG: https://agent-on-rails.suherman.net/downloads/Agent-On-Rails-Setup-macos.dmg |
| **Windows** | MSI (CI): https://agent-on-rails.suherman.net/downloads/Agent-On-Rails-Setup-windows.msi |
| **Linux** | **Build from source** — see [`docs/build-linux.md`](./docs/build-linux.md) (no prebuilt binary) |

Sparkle appcast (macOS): https://agent-on-rails.suherman.net/downloads/appcast.xml

## Develop

Requires: Node 20+, Rust (rustup), Python 3.11+ on the machine you will install into.

```bash
npm install
npm run tauri dev
```

macOS dock / `.app` icons come from the control-plane brand mark (`brand/logo-mark.png`). Regenerate with:

```bash
npx tauri icon ../agent-on-rails-control-plane/brand/logo-mark.png
```

Unit tests (Rust):

```bash
cd src-tauri && cargo test
```

Release bundles (from this machine’s OS):

```bash
npm run tauri build
```

- macOS → `.app` / `.dmg` (signed release pipeline: [`docs/RELEASING.md`](./docs/RELEASING.md))
- Windows → `.msi` / NSIS (GitHub Actions)
- Linux → compile locally ([`docs/build-linux.md`](./docs/build-linux.md))

### Windows signing status

Authenticode signing requires repository secrets `WINDOWS_CERTIFICATE` + `WINDOWS_CERTIFICATE_PASSWORD`. Without them, CI may publish an **unsigned** MSI for convenience.

## Boundaries

See [`AGENTS.md`](./AGENTS.md). Day-to-day orchestration stays in [`agent-on-rails-cli`](https://github.com/agent-on-rails/agent-on-rails-cli).
