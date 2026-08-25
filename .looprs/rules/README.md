# Rules

Plain Markdown files under `.looprs/rules/*.md`, loaded into the agent system
prompt on every session (`RuleRegistry`, skipping any `README.md`).

## Rules in this repo

- `looprs-overview.md` — orients the agent: this is looprs itself (the Rust
  agent runtime), not a downstream consumer of it.
- `code-quality.md` — code quality standards for changes in this repo.
- `security.md` — security guidelines.

Add more `.md` files here to extend the constraint set; each is injected as-is.
