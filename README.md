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
├── commands/              # Custom commands (/)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
├── rules/                 # Constraints and guidelines (Markdown)
└── hooks/                 # Event handlers (pre/post actions)
```

Example command that you'd define:

```
/code:refactor
  Description: Ask AI to refactor selected code
  Template: Refactor this code for readability: {selection}
```

Example skill:

```
$testing
  Level 1: Basic unit test generation
  Level 2: Parametrized tests
  Level 3: Property-based testing
```

The framework is ready to extend. Hook up a parser, wire in the command dispatcher, and you can add unlimited functionality **without changing looprs core**.

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

### Coming
- [ ] Command parser for `/` prefix
- [ ] Skill loader with level tracking
- [ ] Agent dispatcher (YAML-based roles)
- [ ] Rule evaluator (constraint checking)
- [ ] Hook system (event-driven actions)
- [ ] File reference resolver (`@` prefix)
- [ ] Session persistence
- [ ] Plugin system

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
