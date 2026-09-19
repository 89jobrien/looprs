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

## Skill Formats

Nested `SKILL.md` files use YAML frontmatter (delimited by `---`) followed by
the skill instructions. Root-level `.yaml` and `.yml` files use the same
fields with `content` in the YAML document. In both formats, `name`, every
entry in `triggers`, and `content` must contain non-whitespace text;
`description` is optional. Unknown fields are ignored.

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

The equivalent root-level YAML form is:

```yaml
name: rust-error-handling
description: Guide for Rust error handling
triggers:
  - error handling
  - Result type
content: |
  The skill instructions are stored in this field.
```

There is no `hooks`/`commands`/`tools`/`prompt`/`model`/`is_invocable`/
`metadata` schema — that's the public Anthropic Agent Skills spec, not what
this parser implements. Keep skill authoring here to name/description/
triggers/content.

## Notes

- Nested `SKILL.md` files are loaded first, then sorted root YAML files; a root YAML skill overrides a nested Markdown skill with the same name.
- Repo skills take precedence over user skills with the same name.
