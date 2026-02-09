# Rules

This repo does not currently define any rule files under `.looprs/rules/` beyond this README.

If you add rules later, use plain markdown/text files such as:

- `system-rules.md` (core behavior)
- `code-rules.md` (quality/style)
- `security-rules.md` (security constraints)
- `testing-rules.md` (testing requirements)
- `languages/rust-rules.md` (language-specific)

Rules are intended to be loaded into agent system prompts and enforced by tooling, but the repo does not ship rule content yet.
