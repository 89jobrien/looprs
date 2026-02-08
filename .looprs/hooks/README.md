# Hooks

Event-based automation and reactions.

## Directory Structure

```
hooks/
├── pre-command/                # Before command execution
│   ├── validate.yaml           # Validate inputs
│   └── prepare.yaml            # Set up context
├── post-command/               # After successful command
│   ├── cleanup.yaml
│   └── notify.yaml
├── on-error/                   # When errors occur
│   ├── recover.yaml
│   └── diagnose.yaml
├── on-event/                   # General events
│   ├── git-push.yaml          # When git push happens
│   └── file-change.yaml       # When files change
└── README.md
```

## Hook Format

```yaml
name: "hook_name"
description: "What this hook does"
enabled: true

# When this hook triggers
trigger:
  type: "pre-command|post-command|on-error|on-event"
  event: "command_name"         # For on-event, the event name
  
# Optional: Only run if condition is true
condition: |
  {{ event.type }} == "git-push" && {{ has_tests }}

# Actions to perform
actions:
  - type: "message"
    text: "Running pre-push checks..."
  
  - type: "tool"
    name: "bash"
    args:
      cmd: "cargo test"
  
  - type: "conditional"
    if: "{{ exit_code != 0 }}"
    then:
      - type: "message"
        text: "Tests failed, aborting push"
      - type: "block"
        event: "true"
    else:
      - type: "message"
        text: "All checks passed"

# Failure handling
on_failure: "warn|error|continue"

# Order of execution (lower = earlier)
priority: 100
```

## Hook Types

### Pre-Command Hooks
Run before a command executes.

Example: Validate parameters, check prerequisites.

```yaml
name: "validate-lint-params"
trigger:
  type: "pre-command"
  event: "code:lint"
condition: |
  {{ parameters.file }} != ""
actions:
  - type: "tool"
    name: "read"
    args:
      path: "{{ parameters.file }}"
  - type: "conditional"
    if: "{{ file_not_found }}"
    then:
      - type: "error"
        message: "File not found: {{ parameters.file }}"
```

### Post-Command Hooks
Run after successful command completion.

Example: Cleanup, summarize results, next steps.

```yaml
name: "post-test-summary"
trigger:
  type: "post-command"
  event: "test:run"
actions:
  - type: "message"
    text: |
      Tests completed
      Passed: {{ test_results.passed }}
      Failed: {{ test_results.failed }}
      Coverage: {{ test_results.coverage }}
```

### Error Hooks
Run when errors occur.

Example: Diagnosis, recovery attempts.

```yaml
name: "on-bash-error"
trigger:
  type: "on-error"
  event: "bash"
actions:
  - type: "message"
    text: "Command failed with exit code: {{ exit_code }}"
  - type: "agent"
    prompt: "Explain why this command failed and how to fix it"
    context: "{{ error_output }}"
```

### Event Hooks
React to specific events.

Example: Auto-run tests on file change, validate on git push.

```yaml
name: "auto-test-on-file-change"
trigger:
  type: "on-event"
  event: "file-changed"
condition: |
  {{ file_path }}.contains("src/")
actions:
  - type: "message"
    text: "Source changed, running tests..."
  - type: "command"
    name: "test:run"
    args:
      filter: "{{ file_module }}"
```

## Hook Examples

### Pre-Push Validation

`hooks/on-event/pre-push.yaml`:
```yaml
name: "validate-before-push"
trigger:
  type: "on-event"
  event: "git:pre-push"
actions:
  - type: "message"
    text: "Validating code before push..."
  
  - type: "command"
    name: "code:lint"
  
  - type: "conditional"
    if: "{{ exit_code != 0 }}"
    then:
      - type: "error"
        message: "Linting failed, push blocked"
      - type: "block"
        event: "true"
  
  - type: "message"
    text: "✓ All checks passed, proceeding with push"
priority: 10
```

### Auto-Test on File Save

`hooks/on-event/auto-test.yaml`:
```yaml
name: "run-tests-on-change"
trigger:
  type: "on-event"
  event: "file-changed"
condition: |
  {{ file_path }}.contains("src/") && !{{ file_path }}.contains("tests/")
actions:
  - type: "message"
    text: "Running related tests..."
  
  - type: "command"
    name: "test:run"
    args:
      filter: "{{ related_tests }}"
priority: 50
```

## Variables

Available variables in hooks:

| Variable | Type | Description |
|----------|------|-------------|
| `{{ event.type }}` | string | Event type |
| `{{ event.name }}` | string | Event name |
| `{{ parameters }}` | object | Command parameters |
| `{{ exit_code }}` | number | Last command's exit code |
| `{{ error_output }}` | string | Error message |
| `{{ file_path }}` | string | File being modified |

## Next: Implement Hook System

The system will need to:
1. Monitor for trigger events
2. Evaluate conditions with templating
3. Execute action sequences
4. Handle errors and blocking
5. Log hook execution
