# Hooks

Repo-level hooks loaded from `.looprs/hooks/*.yaml`. Repo hooks override user hooks with the same name.

## Hooks in this repo

### `SessionStart.yaml`
- **name**: `session_start`
- **trigger**: `SessionStart`
- **actions**:
  - command: `git --no-pager log -1 --oneline` (injects `last_commit`)
  - command: `git --no-pager status --short` (injects `git_status`)

### `UserPromptSubmit.yaml`
- **trigger**: `UserPromptSubmit`
- **actions**:
  - command: `git --no-pager branch --show-current` (injects `current_branch`)
  - command: `git --no-pager diff --stat HEAD | tail -5` (injects `working_tree_diff`)
- Keeps the model oriented on branch/dirty-tree state without a manual `/git` command.
- Commands run through Nushell, so redirects use `e>`, not POSIX `2>`.

### `InferenceComplete.yaml`
- **name**: `inference_complete`
- **trigger**: `InferenceComplete`
- **actions**: injects a lightweight `inference_event` marker after every response.

### `PostToolUse.yaml`
- **name**: `post_tool_use`
- **trigger**: `PostToolUse`
- **condition**: `"tool_name in ['write', 'edit']"` — **not currently a supported
  condition syntax** (the executor only recognizes `on_branch:`, `has_tool:`,
  `equals:`, `env_set:`, `config_flag:` prefixes; anything else fails closed
  with a warning). This hook does not fire as written — needs either a
  supported condition or an `eval_condition` extension.
- **actions**: runs `cargo clippy --quiet` and injects output as `clippy_output`.

## Format (YAML)

```yaml
name: hook_name
trigger: SessionStart|UserPromptSubmit|InferenceComplete|PreToolUse|PostToolUse|OnError|OnWarning|SessionEnd|DelegationStart|DelegationComplete
actions:
  - type: message
    text: "..."
  - type: command
    command: "..."
    inject_as: "key"
    requires_approval: true
    approval_prompt: "..."
```
