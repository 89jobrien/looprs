# Agents

This repo does not currently define any agent YAML files under `.looprs/agents/` beyond this README.

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
