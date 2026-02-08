# looprs

A unified abstraction layer for agentic AI. Looprs gives you a REPL that talks to LLMs (Claude, GPT, local models) and provides consistent interfaces for extending it with **commands** (`/`), **skills** (`$`), **agents**, **rules**, **hooks**, and **file references** (`@`).

Think of it as: LLM + tools + extensibility framework = everything you need to build agents.

## Install

```bash
git clone https://github.com/89jobrien/looprs.git
cd looprs
cargo build --release
./target/release/looprs
# or: cargo install --path .
```

## Configure

Pick an LLM provider:

```bash
# Anthropic (recommended, fastest setup)
export ANTHROPIC_API_KEY="sk-ant-..."
looprs

# OpenAI (GPT-4/GPT-5)
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4-turbo"
looprs

# Local (Ollama)
ollama serve  # in another terminal
export PROVIDER="local"
looprs
```

Or use `.looprs/provider.json` for persistent config. See `.env.example` for all options.

## Built-In Tools

**File operations:**
- `/read` - read files with line pagination
- `/write` - create/overwrite files
- `/edit` - replace text in files
- `/glob` - find files by name pattern (10-100x faster with `fd` installed)
- `/grep` - search file contents (10-100x faster with `rg` installed)
- `/bash` - execute shell commands

**Optional performance upgrades:**
```bash
cargo install ripgrep      # grep speedup
cargo install fd-find      # glob speedup
```

Both are detected automatically. Falls back to pure Rust if not installed.

## Extensibility Framework

The `.looprs/` directory defines your agent:

```
.looprs/
├── provider.json          # Provider settings
├── config.json            # Global config
├── hooks/                 # Event hooks (context injection, automation)
│   ├── SessionStart       # Inject repo map, jj status, bd list, kanban
│   ├── UserPromptSubmit   # Enrich context before LLM
│   ├── PreToolUse         # Approval gates
│   └── PostToolUse        # Sync to bd, kanban, etc.
├── commands/              # Custom commands (/)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
└── rules/                 # Constraints and guidelines (Markdown)
```

Example hook that injects context:

```yaml
# .looprs/hooks/SessionStart
events: [SessionStart]
actions:
  - exec: jj log --no-pager -r 'main::' | head -5
    inject_as: recent_commits
  - exec: bd list --open
    inject_as: open_issues
  - exec: kanban_board --json
    inject_as: board_state
```

Example command:

```
/code:refactor
  Description: Ask AI to refactor selected code
  Template: Refactor this code for readability: {selection}
```

The framework is ready to extend. Define hooks, commands, skills - all **without changing looprs core**.

## Multi-Provider LLM

Looprs works with any major LLM:

| Provider | Setup | Models | Cost |
|----------|-------|--------|------|
| Anthropic | `ANTHROPIC_API_KEY` | Claude 3 (Opus/Sonnet/Haiku) | $$ |
| OpenAI | `OPENAI_API_KEY` | GPT-4/GPT-5 | $$$ |
| Local | `ollama serve` | llama2, mistral, neural-chat, etc. | Free |

**Auto-detects** from env vars. Force with `PROVIDER=anthropic \| openai \| local`.

Per-provider config: `.looprs/provider.json` or `MODEL=` env var.

## Roadmap

### Done ✅
- [x] Multi-provider LLM support (Anthropic, OpenAI, Local)
- [x] Fast search: grep + ripgrep, glob + fd
- [x] Extensibility framework (commands, skills, agents, rules, hooks, file refs)
- [x] Provider configuration (env vars + config file)

### Phase 2: Context & Workflow Integration
- [ ] **Hook system** - SessionStart, SessionEnd, PreToolUse, PostToolUse, OnError, OnWarning
  - Inject context before LLM calls (repo map, status, issues, kanban state)
  - Approval gates for automated actions
  - Pre-defined prompts and setup system
- [ ] **jj integration** - Read repo state, query branches, diff, log
- [ ] **bd integration** - List issues, query tasks, sync with hooks
- [ ] **Kanban board** - SQLite → bd issues bridge, real-time sync

### Phase 3: Extensibility Parsers
- [ ] Command parser for `/` prefix
- [ ] Skill loader with level tracking
- [ ] Agent dispatcher (YAML-based roles)
- [ ] Rule evaluator (constraint checking)
- [ ] File reference resolver (`@` prefix)

### Phase 4: Advanced Features
- [ ] Session persistence (conversation history)
- [ ] Plugin system (custom tools/commands)
- [ ] Performance profiling
- [ ] Concurrent hook execution

## Dev

```bash
make build      # build release binary
make test       # run tests
make lint       # run clippy
make install    # install locally
```

Uses `prek` for pre-commit hooks (cargo test + clippy). See `Makefile` for all targets.

## License

MIT
