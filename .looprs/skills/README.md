# Skills

Modular, self-contained packages that extend looprs capabilities with specialized knowledge, workflows, and tool integrations. Skills follow the Anthropic Agent Skills standard.

## About Skills

Skills are "onboarding guides" for specific domains or tasks—they transform looprs from a general-purpose agent into a specialized agent equipped with procedural knowledge.

**Core principle:** Default assumption is that Claude is already very smart. Only add context Claude doesn't already have.

## Directory Structure

Skills are organized by category in subdirectories:

```
skills/
├── rust/                       # Rust-specific workflows
│   ├── error-handling/
│   │   ├── SKILL.md           # Main skill file
│   │   ├── scripts/           # Executable code (optional)
│   │   ├── references/        # Documentation (optional)
│   │   └── assets/            # Templates/files (optional)
│   └── testing/
│       └── SKILL.md
├── development/                # General development
│   ├── debugging/
│   └── refactoring/
└── README.md                   # This file
```

## Skill Format

Each skill is a folder containing a `SKILL.md` file with YAML frontmatter:

```markdown
---
name: skill-name
description: Complete description of what this skill does and when to use it
---

# Skill Name

Concise instructions for Claude to follow (<500 lines preferred).

## Quick Start
[Basic usage pattern]

## Workflows
[Step-by-step procedures]

## Advanced
[Edge cases, optimization, troubleshooting]
```

### Required Fields

- **name**: Unique identifier (lowercase, hyphens for spaces)
- **description**: Complete description of what this skill does and when to use it
- **triggers**: List of keywords/phrases that activate this skill (explicit triggering)

Example:
```yaml
---
name: rust-error-handling
description: Guide for Rust error handling with Result<T,E>, error propagation, and custom error types
triggers:
  - "error handling"
  - "Result type"
  - "? operator"
  - "thiserror"
  - "anyhow"
---
```

### Optional Bundled Resources

**scripts/** - Executable code (Python/Bash/etc.)
- Use when code is repeatedly rewritten or needs deterministic reliability
- May be executed without loading into context
- Example: `scripts/format_code.py`

**references/** - Documentation loaded as needed
- Use for detailed information that shouldn't bloat SKILL.md
- Only loaded when Claude determines it's needed
- Example: `references/api_docs.md`, `references/patterns.md`

**assets/** - Files used in output (not loaded into context)
- Templates, images, boilerplate that gets copied/modified
- Example: `assets/template.rs`, `assets/logo.png`

## Design Principles

### 1. Concise is Key

Context window is a public good. Challenge each piece of information:
- "Does Claude really need this explanation?"
- "Does this paragraph justify its token cost?"

Prefer concise examples over verbose explanations.

### 2. Progressive Disclosure

Three-level loading system:
1. **Metadata (name + description)** - Always in context (~100 words)
2. **SKILL.md body** - When skill triggers (<500 lines)
3. **Bundled resources** - As needed by Claude

Keep SKILL.md under 500 lines. Split content into reference files when approaching this limit.

### 3. Set Appropriate Degrees of Freedom

Match specificity to task fragility:
- **High freedom**: Multiple approaches valid, use text instructions
- **Medium freedom**: Preferred pattern exists, use pseudocode/parameterized scripts
- **Low freedom**: Operations fragile/critical, use specific scripts with few parameters

## Usage

Skills are automatically triggered when user messages contain trigger keywords/phrases. You can also explicitly invoke:

```
Use the rust-error-handling skill to refactor this code
$rust-error-handling     # Explicit invocation
```

**Trigger matching:**
- Case-insensitive substring matching
- Multiple triggers per skill (OR logic)
- User message: "How do I use the ? operator?" → triggers rust-error-handling
- User message: "Need help with Result types" → triggers rust-error-handling

Skills can reference bundled resources:
```
See references/patterns.md for advanced examples
Run scripts/format_code.py to apply formatting
```

## Example: Rust Error Handling

`skills/rust/error-handling/SKILL.md`:
```markdown
---
name: rust-error-handling
description: Guide for Rust error handling with Result<T,E>, error propagation, and custom error types
triggers:
  - "error handling"
  - "Result type"
  - "? operator"
  - "thiserror"
  - "anyhow"
  - "error propagation"
---

# Rust Error Handling

## Quick Start

Use `Result<T, E>` for operations that can fail:
\`\`\`rust
fn parse_config(path: &str) -> Result<Config, ConfigError> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
\`\`\`

## Error Propagation

The `?` operator automatically converts and returns errors:
- Returns early on Err
- Requires From<SourceError> for TargetError
- Only works in functions returning Result or Option

## Custom Error Types

Use `thiserror` for user-facing errors, `anyhow` for internal errors.

For detailed patterns, see references/error_patterns.md
```

`skills/rust/error-handling/references/error_patterns.md`:
```markdown
# Rust Error Patterns

## Using thiserror
[Detailed examples...]

## Using anyhow
[Detailed examples...]

## Error Context
[Detailed examples...]
```

## Creating Skills

See `skill-creator` skill in Anthropic's marketplace for detailed guidance on creating effective skills.

Key steps:
1. Understand concrete usage examples
2. Plan reusable resources (scripts, references, assets)
3. Write concise SKILL.md with clear description
4. Test with real usage
5. Iterate based on feedback
