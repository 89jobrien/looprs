# Design Specification: Demo Onboarding Wizard (Hooks)

**Author:** Codex
**Date:** 2026-02-09
**Status:** DRAFT
**Reviewers:** TBD

---

## Overview

### Problem Statement

We need an embedded demo onboarding flow that guides users through provider setup using looprs hooks and conventions. It should cover all providers, rely mostly on approvals, allow session-only API key setup, and let users disable onboarding once they complete or skip it.

### Goals

- Provide a hook-driven, wizard-like onboarding flow that runs once by default.
- Allow users to set API keys for the current looprs session without persisting secrets.
- Provide clear instructions for persistent setup without writing secrets.
- Allow users to disable onboarding via `.looprs/config.json`.

### Non-Goals

- Persisting API keys or modifying shell rc files automatically.
- Building a new UI outside the existing hook + REPL conventions.

---

## Background

### Context

Looprs supports hooks with message/command/conditional actions. SessionStart hooks run in the REPL and can require approvals, but they cannot collect user input or update config/env variables today.

### Current State

Repo hooks exist under `.looprs/hooks/`, including demo hooks. The hook system can inject outputs into event context and display them, but there is no safe way to capture user input or set process env/config via hooks.

### User Research / Requirements

| Requirement | Source | Priority |
|-------------|--------|----------|
| Wizard-like onboarding using hooks | User request | P0 |
| Covers Anthropic, OpenAI, Local providers | User request | P0 |
| Session-only API key setup | User request | P0 |
| User can disable onboarding | User request | P0 |
| Persistent setup guidance provided | User request | P1 |

---

## Proposed Solution

### High-Level Design

Add minimal hook primitives for interactive onboarding and configure a repo hook as the “demo” wizard.

```
SessionStart
  -> HookRegistry (repo hooks)
    -> HookExecutor
      -> confirm/prompt/secret_prompt
      -> set_env (process only)
      -> set_config (.looprs/config.json)
```

### User Flow

1. User starts looprs.
2. System runs the demo onboarding hook if `onboarding.demo_seen` is false.
3. User approves or skips provider setup steps (Anthropic, OpenAI, Local).
4. If approved, user enters API key (hidden) for session-only use.
5. System sets env var for the current process and prints persistent setup instructions.
6. User disables onboarding (explicit step), which sets `onboarding.demo_seen=true`.

### Data Model

```json
{
  "onboarding": {
    "demo_seen": true
  }
}
```

---

## Detailed Design

### Component 1: Hook Action Extensions

**Purpose:** Enable interactive wizard steps in hooks without adding new UI layers.

**Implementation:**

- Add new `Action` variants:
  - `confirm`: asks for approval and stores boolean in hook-local context.
  - `prompt`: text input stored in hook-local context.
  - `secret_prompt`: hidden input stored in hook-local context (never displayed).
  - `set_env`: sets a process env var from a stored value.
  - `set_config`: updates `.looprs/config.json` (e.g., `onboarding.demo_seen`).

**Interactions:**

- Receives: user input via REPL prompt callbacks.
- Produces: hook-local key/value entries and config/env side effects.
- Depends on: new callback types injected by the REPL for prompt/secret input.

### Component 2: Hook Conditions

**Purpose:** Allow onboarding flow to short-circuit and skip steps.

**Implementation:**

Add condition evaluation support:
- `config_flag:<path>=<value>` (ex: `config_flag:onboarding.demo_seen=false`)
- `env_set:VAR`
- `equals:<key>:<value>` for hook-local values (ex: `equals:use_openai:true`)
- existing `has_tool:ollama` remains

### Component 3: Demo Onboarding Hook

**Purpose:** Provide the embedded wizard as a repo-level demo hook.

**Implementation:**

Add `.looprs/hooks/demo_onboarding.yaml` with:
- initial condition: `config_flag:onboarding.demo_seen=false`
- a “skip/disable” confirmation step that sets `demo_seen=true`
- per-provider steps (Anthropic, OpenAI, Local)
- persistent setup guidance messages

Sample (trimmed):

```yaml
name: demo_onboarding
trigger: SessionStart
condition: config_flag:onboarding.demo_seen=false
actions:
  - type: message
    text: "Welcome to the demo onboarding wizard. Keys are session-only."
  - type: confirm
    prompt: "Skip onboarding and disable for future sessions?"
    set_key: disable_onboarding
  - type: conditional
    condition: equals:disable_onboarding:true
    then:
      - type: set_config
        path: onboarding.demo_seen
        value: true
  - type: conditional
    condition: equals:disable_onboarding:false
    then:
      - type: confirm
        prompt: "Set up Anthropic now?"
        set_key: use_anthropic
      - type: conditional
        condition: equals:use_anthropic:true
        then:
          - type: secret_prompt
            prompt: "Enter ANTHROPIC_API_KEY"
            set_key: anthropic_key
          - type: set_env
            name: ANTHROPIC_API_KEY
            from_key: anthropic_key
          - type: message
            text: "To persist: export ANTHROPIC_API_KEY=... in your shell profile."
      - type: confirm
        prompt: "Set up OpenAI now?"
        set_key: use_openai
      - type: conditional
        condition: equals:use_openai:true
        then:
          - type: secret_prompt
            prompt: "Enter OPENAI_API_KEY"
            set_key: openai_key
          - type: set_env
            name: OPENAI_API_KEY
            from_key: openai_key
          - type: message
            text: "To persist: export OPENAI_API_KEY=... or use .looprs/provider.json."
      - type: conditional
        condition: has_tool:ollama
        then:
          - type: confirm
            prompt: "Use local provider (Ollama)?"
            set_key: use_local
          - type: message
            text: "To persist: export PROVIDER=local and MODEL=..."
      - type: set_config
        path: onboarding.demo_seen
        value: true
```

### State Management

Hook-local context (not displayed, not injected into LLM prompt):

| Key | Type | Example | Purpose |
|-----|------|---------|---------|
| use_openai | bool | true | Gate provider steps |
| openai_key | string | (redacted) | Source for set_env |

| State | Type | Initial | Transitions |
|------|------|---------|-------------|
| onboarding.demo_seen | bool | false | true after wizard completes or user skips |

### Error Handling

| Error Case | Handling | User Message |
|------------|----------|--------------|
| User cancels prompt | Skip step | "Skipped setup" |
| Empty key entered | Skip env set | "Key not set" |
| Non-interactive mode | Skip prompts | "Onboarding skipped in non-interactive mode" |

---

## Alternatives Considered

### Alternative 1: Hook-only messages (no interactive actions)

**Pros:** No code changes.

**Cons:** Cannot capture keys or set env; wizard feels manual.

**Why Not:** Does not meet “session-only key setup” requirement.

### Alternative 2: Dedicated `/onboard` command

**Pros:** Rich flow and branching.

**Cons:** Less aligned with “hooks and conventions”; more bespoke code.

**Why Not:** The request is to embed as a demo hook.

---

## Security Considerations

### Threats

| Threat | Risk | Mitigation |
|--------|------|------------|
| Secret leaks in logs | M | Redact `secret_prompt` values and exclude from hook-injected context output |
| Accidental persistence | L | Only set process env vars; never write secrets to disk |

### Data Privacy

- Do not store keys in config or files.
- Do not echo secret inputs to the UI.

---

## Performance Considerations

### Expected Load

| Metric | Expected | Peak |
|--------|----------|------|
| SessionStart hook execution | Minimal | Minimal |
| Prompt latency | User-driven | User-driven |

### Scalability

N/A (interactive flow only).

### Caching Strategy

None.

---

## Testing Strategy

### Unit Tests

- Hook action parsing for `confirm`, `prompt`, `secret_prompt`, `set_env`, `set_config`.
- Condition evaluation for `config_flag` and `env_set`.
- Ensure secret values are not injected into display metadata.

### Integration Tests

- Onboarding hook sets `onboarding.demo_seen=true` in `.looprs/config.json`.
- SessionStart skips wizard when flag is true.

### Edge Cases

- Non-interactive mode (scriptable) skips prompts safely.
- Missing `.looprs/` directory is created on config save.

---

## Rollout Plan

### Phase 1: Demo Hook + Hook Primitives

- Deliverable: new hook actions/conditions and demo hook file.
- Audience: repo users.

### Feature Flags

| Flag | Purpose | Default |
|------|---------|---------|
| `onboarding.demo_seen` | skip demo wizard | false |

### Rollback Criteria

- Any secret leakage into displayed context.
- Hook prompt crashes in interactive mode.

---

## Monitoring & Observability

### Metrics

| Metric | Purpose | Alert Threshold |
|--------|---------|-----------------|
| Wizard skip rate | UX check | N/A |

### Logging

| Event | Level | Data |
|-------|-------|------|
| Onboarding start/finish | INFO | provider selections (no secrets) |

---

## Dependencies

### External Dependencies

| Dependency | Purpose | Owner |
|------------|---------|-------|
| None | N/A | N/A |

### Internal Dependencies

- `src/hooks/` executor and parser changes
- `.looprs/config.json` schema extension

---

## Open Questions

1. Should the wizard default to setting `:set provider` for the active session?
2. Should we allow a “re-run onboarding” command (e.g., reset flag)?

---

## Timeline

| Milestone | Date |
|-----------|------|
| Design Review | 2026-02-09 |
| Implementation Start | TBD |
| Testing Complete | TBD |
| Rollout | TBD |

---

## References

- `.looprs/hooks/README.md`
- `src/hooks/executor.rs`
- `src/bin/looprs/main.rs`

---

## Quality Checklist

- [x] Problem clearly stated
- [x] Goals and non-goals defined
- [x] Alternatives considered
- [x] Security reviewed
- [x] Performance considered
- [x] Testing strategy defined
- [x] Rollout plan documented
- [ ] Monitoring planned
