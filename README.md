# Agent On Rails — Desktop Setup (macOS / Windows)

Thin **setup wizard** for first-time operators (control-plane [AOR-008](https://github.com/agent-on-rails/agent-on-rails-control-plane/blob/main/specs/AOR-008-desktop-setup/spec.md), [ADR-008](https://github.com/agent-on-rails/agent-on-rails-control-plane/blob/main/adr/ADR-008-desktop-setup-wizard.md)).

It does **not** replace the `aor` CLI/TUI. Flow:

1. Check Python 3.11+ / pipx / `aor`
2. Install or upgrade the CLI from `agent-on-rails-cli`
3. Run `aor init` for a docs+specs project
4. Hand off to [walkthrough](https://github.com/agent-on-rails/agent-on-rails-cli/blob/main/docs/walkthrough.md)

Stack: **Tauri 2** + TypeScript (vanilla).

## Develop

Requires: Node 20+, Rust (rustup), Python 3.11+ on the machine you will install into.

```bash
npm install
npm run tauri dev
```

Unit tests (Rust):

```bash
cd src-tauri && cargo test
```

Release bundles (from this machine’s OS):

```bash
npm run tauri build
```

- macOS → `.app` / `.dmg`
- Windows → `.msi` / NSIS (build on Windows)

## Boundaries

See [`AGENTS.md`](./AGENTS.md). Day-to-day orchestration stays in [`agent-on-rails-cli`](https://github.com/agent-on-rails/agent-on-rails-cli).
