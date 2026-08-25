# Repo Configuration (.looprs)

Repo-level configuration for looprs. These files are loaded in addition to user-level configs under `~/.looprs/`, with **repo precedence** (repo entries override user entries with the same name).

## Structure (this repo)

```
.looprs/
├── commands/                 # Custom slash commands (YAML) — see commands/README.md
├── hooks/                    # Repo hooks (YAML) — see hooks/README.md
├── agents/                   # Agent role definitions (YAML) — see agents/README.md
├── rules/                    # Constraint guidelines (Markdown) — see rules/README.md
├── skills/                   # Example skills (see skills/README.md)
├── scripts/                  # Helper scripts invoked by commands/hooks
├── observability/            # JSONL traces/events (project-scoped; see root README's Observability section)
├── config.json               # Runtime defaults, file refs, pipeline, agents, paths
├── provider.json              # Active provider/model settings (gitignored)
├── provider.json.example     # Provider config template
└── provider-config.md        # Provider config notes
```

(File lists inside each directory aren't reproduced here — they change
often; see the directory itself or its own `README.md`.)

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
- `persistence`: session store backend (`sqlite` at `~/.looprs/sessions.db`, or `fs` (default) at `~/.looprs/sessions/`).

Provider selection, model IDs, `max_tokens`, and provider API timeouts belong in `.looprs/provider.json`, not `config.json`.

## Pointers

- Commands: see `./commands/README.md`
- Hooks: see `./hooks/README.md`
- Skills: see `./skills/README.md`
