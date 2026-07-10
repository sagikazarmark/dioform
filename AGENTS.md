# Agents

## Minimum verification

Changes made at the very least must pass these checks:

- cargo fmt --check
- cargo clippy --all-targets --all-features --workspace
- cargo test --all-targets --all-features --workspace

## Agent skills

### Issue tracker

Issues and PRDs live in GitHub Issues for `sagikazarmark/dioform`; use the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the canonical triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: read root `CONTEXT.md` and root `docs/adr/` when present. See `docs/agents/domain.md`.
