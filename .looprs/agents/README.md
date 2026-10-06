# Agents

This repo currently defines these agent YAML files under `.looprs/agents/`:

- `opencode.yaml`
- `planner.yaml`
- `reviewer.yaml`
- `taskit.yaml`

`AgentRegistry` is fully wired; `config.json`'s `agents.delegate_by_default` is
currently `false` (delegation is opt-in per session/command) with `max_parallel: 3`.

If you add agents later, place YAML definitions here. Example structure:

```yaml
name: "code-reviewer"
role: "Senior Code Reviewer"
description: "Reviews code for quality and correctness"
system_prompt: |
  You are a senior code reviewer...
tools:
  - read
  - edit
skills:
  - $rust-idioms
constraints:
  - "Highlight security issues"
```

Agents can be wired up by commands or future orchestration logic in looprs.
