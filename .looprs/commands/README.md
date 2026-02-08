# Commands

Custom commands invoked with `/` prefix.

## Directory Structure

```
commands/
├── code/              # Code-related commands
│   ├── lint.yaml
│   ├── test.yaml
│   └── format.yaml
├── docs/              # Documentation commands
│   ├── explain.yaml
│   └── document.yaml
├── git/               # Git workflow commands
│   ├── commit.yaml
│   └── review.yaml
└── README.md          # Command documentation
```

## Command Format

```yaml
name: "command_name"
description: "Brief description"
category: "group"                    # Optional: groups into /group:command
usage: "/command [args]"
parameters:
  - name: "param_name"
    description: "What it does"
    type: "string|number|boolean"
    required: true|false
    default: "value"
    enum: ["option1", "option2"]
actions:
  - type: "tool"                     # tool|agent|message|conditional
    name: "tool_name"
    args: {}
  - type: "message"
    text: "Output text"
aliases:
  - "cmd"                            # Alternative invoke name
```

## Usage

- `/command` - Simple invocation
- `/group:command` - Nested groups
- `/group:sub:command` - Multiple levels
- `/command -p value` - With parameters

## Examples

### Simple Command

`commands/help.yaml`:
```yaml
name: "help"
description: "Show available commands"
actions:
  - type: "message"
    text: "Available commands:\n- /code:lint - Lint files\n- /git:commit - Commit changes"
```

Usage: `/help`

### Grouped Command with Parameters

`commands/code/lint.yaml`:
```yaml
name: "lint"
category: "code"
description: "Lint and fix code issues"
parameters:
  - name: "file"
    description: "File or glob pattern to lint"
    type: "string"
    required: true
actions:
  - type: "tool"
    name: "read"
    args:
      path: "{{ parameters.file }}"
  - type: "agent"
    prompt: "Lint this code using clippy and rustfmt standards"
  - type: "message"
    text: "Linting complete"
```

Usage: `/code:lint src/main.rs` or `/code:lint @src/main.rs`

## Next: Implement Command Parser

The CLI will need to:
1. Parse `/` prefix for commands
2. Support grouped naming: `group:command:subcommand`
3. Handle parameter binding
4. Execute action sequences
