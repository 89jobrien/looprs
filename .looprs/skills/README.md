# Skills

<!-- IDEA(M4): deploy looprs-specific skills. Two high-value targets:
- `looprs-architecture` — triggers on "hexagonal", "port", "agent loop", "registry"
- `looprs-testing` — triggers on "mock provider", "proptest", "contract test"
Examples live in examples/; copy one and adapt. -->

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

Current, concise: 

```markdown
---
name: rust-error-handling
description: Guide for Rust error handling
triggers:
  - "error handling"
  - "Result type"
---
```

```
hooks: list[str]
commands: list[str]
tools: list[str]
prompt: str
model: str
is_invocable: bool
is_discoverable: bool
metadata: 
  - version: 0.0.0
  - author: John Doe
```

## Notes

- Examples live under `examples/` only; there are no active repo-specific skills beyond these samples.
- Repo skills (if added later) take precedence over user skills with the same name.
