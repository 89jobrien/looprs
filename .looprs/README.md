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
├── config.json               # Repo config (loaded by looprs)
├── provider.json.example     # Provider config template
└── provider-config.md        # Provider config notes
```

## Precedence

- **Commands**: repo commands override user commands with the same name.
- **Hooks**: repo hooks override user hooks with the same name.
- **Skills**: repo skills take precedence over user skills when names collide.

## Pointers

- Commands: see `./commands/README.md`
- Hooks: see `./hooks/README.md`
- Skills: see `./skills/README.md`
