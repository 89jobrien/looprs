# Looprs Configuration Directory

This directory contains all extensibility configurations for looprs.

## Directory Structure

```
.looprs/
├── commands/           # Slash commands (/command or /group:command)
├── skills/            # Reusable skills ($skill_name)
├── agents/            # Agent definitions and orchestration
├── rules/             # System rules and constraints
├── hooks/             # Event-based automation
├── config.json        # Global configuration
└── README.md          # This file
```

## Components Overview

### Commands (`/commands`)

Slash commands invoked from the REPL with `/` prefix.

**Invocation styles:**
- `/command` - Simple command
- `/group:command` - Nested command (group:subcommand)
- `/group:subgroup:command` - Multi-level nesting

**Structure:** YAML files with metadata, actions, and parameter schemas
```yaml
name: "command-name"
description: "What this command does"
category: "group"  # Optional: for /group:command organization
parameters:
  - name: "param1"
    description: "Parameter description"
    type: "string"
    required: true
actions:
  - type: "tool"
    name: "read"
    args: "..."
  - type: "message"
    text: "Output message"
```

### Skills (`/skills`)

Reusable, composable skills that practice progressive disclosure.

**Invocation:**
- `$skill_name` - Loads and applies skill
- Skills can reference other skills: `$skill_a` calls `$skill_b`

**Structure:** JSON with progressive stages
```json
{
  "name": "skill_name",
  "description": "What this skill teaches/does",
  "category": "code|documentation|testing|refactoring",
  "stages": [
    {
      "level": 1,
      "title": "Foundation",
      "description": "Basic pattern",
      "examples": ["example1", "example2"]
    },
    {
      "level": 2,
      "title": "Advanced",
      "description": "Advanced usage",
      "examples": ["example1", "example2"]
    }
  ]
}
```

### Agents (`/agents`)

Agent definitions for different roles and specializations.

**Invocation:**
- Agents are invoked internally or via commands
- Can orchestrate other agents

**Structure:** YAML with personality, system prompts, tool access
```yaml
name: "agent_name"
role: "Senior Backend Developer"
description: "Specialized in backend development"
system_prompt: "You are a..."
tools:
  - read
  - write
  - edit
  - bash
specialized_skills:
  - $skill_a
  - $skill_b
```

### Rules (`/rules`)

System rules and constraints that govern behavior.

**Files:** Plain text/markdown with rules
- `system-rules.md` - Core system behavior
- `code-rules.md` - Code quality rules
- `security-rules.md` - Security constraints
- Language-specific: `python-rules.md`, `rust-rules.md`, etc.

### Hooks (`/hooks`)

Event-based automation triggered by specific conditions.

**Structure:** YAML with event triggers and actions
```yaml
name: "hook_name"
trigger: "event_type"  # pre-command, post-command, on-error, on-success
condition: "some_condition"
actions:
  - type: "message"
    text: "..."
  - type: "tool"
    name: "..."
```

## File References with `@`

Reference local files in REPL:

```
@path/to/file     # Read from cwd
@./relative/path  # Relative path
@/absolute/path   # Absolute path
```

The `@` syntax allows you to:
- Pass file content to agent: `@file.rs help me refactor this`
- Reference in commands: `/lint @src/main.rs`
- Include in skills: `$review @module.py`

## Examples

### Command Example

File: `.looprs/commands/code-tools.yaml`
```yaml
name: "lint"
description: "Lint code in a file"
category: "code-tools"
parameters:
  - name: "file"
    description: "File to lint"
    type: "string"
    required: true
actions:
  - type: "tool"
    name: "read"
    args:
      path: "{{ file }}"
  - type: "agent"
    prompt: "Lint this code and provide feedback"
```

Usage: `/code-tools:lint @src/main.rs`

### Skill Example

File: `.looprs/skills/rust-error-handling.json`
```json
{
  "name": "rust-error-handling",
  "description": "Mastering Result and Error types",
  "category": "rust",
  "stages": [
    {
      "level": 1,
      "title": "Basics",
      "description": "Understanding Result<T, E>",
      "examples": ["ok_example", "err_example"]
    }
  ]
}
```

Usage: `$rust-error-handling` (loads skill)

## Getting Started

1. Create commands in `commands/` directory as YAML files
2. Add skills in `skills/` directory as JSON files  
3. Define agents in `agents/` directory as YAML files
4. Add rules in `rules/` directory as markdown
5. Create hooks in `hooks/` directory as YAML files
6. Reference files with `@path/to/file` in the REPL
7. Use commands with `/command` or `/group:command`
8. Apply skills with `$skill_name`

## Next Steps

- Implement command loader and `/` prefix parser
- Implement skill loader and `$` prefix handler
- Implement `@` file reference resolver
- Build agent orchestration system
- Add hook event system
