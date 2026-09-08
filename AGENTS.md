# AGENTS.md — agent-on-rails-desktop

Read control-plane [`AGENTS.md`](https://github.com/agent-on-rails/agent-on-rails-control-plane/blob/main/AGENTS.md) first.

Governing contracts: **ADR-008**, **AOR-008**.

## Boundaries

- **Setup / onboarding only**: prerequisites, install `aor`, `aor init`, links to docs.
- Do **not** re-implement planner, review loop, evidence, or GitHub App logic.
- Do **not** approve specs or mark tasks `DONE` from this app.
- After setup, operators use the CLI/TUI (`aor`) and GitHub.

## Prohibited

- Storing API keys or secrets in the app bundle or source
- Building a full desktop operator console here (that is a later product surface)
- Bypassing human-approval policies
