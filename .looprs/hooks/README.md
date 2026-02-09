# Hooks

Repo-level hooks loaded from `.looprs/hooks/*.yaml`. Repo hooks override user hooks with the same name.

## Hooks in this repo

### `SessionStart.yaml`
- **name**: `project_info`
- **trigger**: `SessionStart`
- **actions**:
  - message: "Loaded from repo-level hooks"
  - command: `git --no-pager log -1 --oneline` (injects `last_commit`)

### `demo_approval.yaml`
- **name**: `demo_approval_gate`
- **trigger**: `SessionStart`
- **actions**:
  - message: welcome text
  - command: `git --no-pager log -1 --oneline` (injects `last_commit`)
  - command: `git --no-pager status --short` (requires approval, injects `git_status`)

## Format (YAML)

```yaml
name: hook_name
trigger: SessionStart|SessionEnd|PreToolUse|PostToolUse|OnError|OnWarning
actions:
  - type: message
    text: "..."
  - type: command
    command: "..."
    inject_as: "key"
    requires_approval: true
    approval_prompt: "..."
```
