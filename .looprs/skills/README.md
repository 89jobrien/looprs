# Skills

This repo contains **example** skills under `.looprs/skills/examples/`. They demonstrate the Anthropic Agent Skills layout used by looprs.

## Structure (this repo)

```
skills/
└── examples/
    ├── rust-error-handling/
    │   ├── SKILL.md
    │   ├── references/
    │   └── scripts/
    └── rust-testing/
        └── SKILL.md
```

## Skill Format

`SKILL.md` includes YAML frontmatter plus concise instructions:

```markdown
---
name: rust-error-handling
description: Guide for Rust error handling
triggers:
  - "error handling"
  - "Result type"
---

# Rust Error Handling
...
```

## Notes

- Examples live under `examples/` only; there are no active repo-specific skills beyond these samples.
- Repo skills (if added later) take precedence over user skills with the same name.
