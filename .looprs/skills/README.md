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

`SKILL.md` is YAML frontmatter (delimited by `---`) followed by the skill's
instructions as plain content. The parser (`skills/parser.rs`) only reads
three frontmatter fields — `name` and `triggers` are required, `description`
is optional. Anything else in frontmatter is ignored, not an error:

```markdown
---
name: rust-error-handling
description: Guide for Rust error handling
triggers:
  - "error handling"
  - "Result type"
---

The rest of the file is the skill's content, injected verbatim when
triggered.
```

There is no `hooks`/`commands`/`tools`/`prompt`/`model`/`is_invocable`/
`metadata` schema — that's the public Anthropic Agent Skills spec, not what
this parser implements. Keep skill authoring here to name/description/
triggers/content.

## Notes

- Examples live under `examples/` only; there are no active repo-specific skills beyond these samples.
- Repo skills (if added later) take precedence over user skills with the same name.
