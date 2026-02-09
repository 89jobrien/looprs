# Commands

Repo-level custom commands loaded from `.looprs/commands/*.yaml`.

## Commands in this repo

| Command | Aliases | Action | Notes |
|--------|---------|--------|-------|
| `/help` | `/h` | message | Lists available custom commands |
| `/refactor` | `/r` | prompt | Sends a refactor prompt to the LLM |
| `/test` | `/t` | shell | `cargo test --lib` (output injected) |
| `/lint` | `/l` | shell | `cargo clippy --all-targets -- -D warnings` (output injected) |

## Format (YAML)

```yaml
name: command
description: Short description
aliases:
  - alias
action:
  type: message|prompt|shell
  # for prompt:
  template: "..."
  # for shell:
  command: "..."
  inject_output: true
```

## Usage

```
/help
/refactor
/test
/lint
```
