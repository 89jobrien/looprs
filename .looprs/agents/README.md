<!-- IDEA(Q4): deploy planner.yaml and reviewer.yaml here. The AgentRegistry is
fully wired and config.json has delegate_by_default: true and max_parallel: 3,
but .looprs/agents/ has no YAML files. Start with 2-3 role definitions. -->

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
