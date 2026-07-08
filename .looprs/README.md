# Repo Configuration (.looprs)

Repo-level configuration for looprs. These files are loaded in addition to user-level configs under `~/.looprs/`, with **repo precedence** (repo entries override user entries with the same name).

## Structure (this repo)

```
.looprs/
├── commands/                 # Custom slash commands (YAML)
│   ├── help.yaml
│   ├── refactor.yaml
│   ├── test.yaml
│   └── lint.yaml
├── hooks/                    # Repo hooks (YAML)
│   ├── SessionStart.yaml
│   ├── demo_approval.yaml
│   └── demo_onboarding.yaml
├── skills/                   # Example skills (see skills/README.md)
│   └── examples/
├── config.json               # Runtime defaults, file refs, pipeline, agents, paths
├── provider.json.example     # Provider config template
└── provider-config.md        # Provider config notes
```

## Precedence

- **Commands**: repo commands override user commands with the same name.
- **Hooks**: repo hooks override user hooks with the same name.
- **Skills**: repo skills take precedence over user skills when names collide.

## Active `config.json` schema

`config.json` is deserialized into `AppConfig` and currently supports these top-level sections:

- `defaults`: runtime defaults such as `max_context_tokens`, `temperature`, and `timeout_seconds`.
- `file_references`: `@file` reference policy, including allowed extensions and maximum file size.
- `onboarding`: repo onboarding state. Runtime state in `.looprs/state.json` can override this value.
- `pipeline`: optional self-improvement pipeline settings, checks, compaction, and log directory.
- `agents`: role delegation settings, parallelism limit, orchestration strategy, filesystem mode, and optional default agent.
- `paths`: repo-local extension directories for agents, commands, hooks, rules, and skills.

Provider selection, model IDs, `max_tokens`, and provider API timeouts belong in `.looprs/provider.json`, not `config.json`.

## Pointers

- Commands: see `./commands/README.md`
- Hooks: see `./hooks/README.md`
- Skills: see `./skills/README.md`
