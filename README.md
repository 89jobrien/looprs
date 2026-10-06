# looprs

A Rust LLM agent loop CLI. Tools, loops, and conditions — no convoluted markdown parsing system.

## Install

```bash
git clone https://github.com/89jobrien/looprs.git
cd looprs
cargo build --release
./target/release/looprs
# or: cargo install --path crates/looprs-cli
```

## Local env

```bash
cp .envrc.example .envrc
direnv allow
```

## Configure

Pick a provider:

```bash
# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
looprs

# OpenAI
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4-turbo"
looprs

# Local (Ollama)
ollama serve  # in another terminal
export PROVIDER="local"
looprs

# Gemini
export GEMINI_API_KEY="..."
export PROVIDER="gemini"          # or: google
looprs

# SDK-backed providers
export PROVIDER="openai-sdk"      # openai-sdk | anthropic-sdk | claude-sdk
looprs
```

Persistent config: `.looprs/provider.json`. All env options: `.env.example`.

## Built-in Tools

These are tool capabilities exposed to the model during a session (not slash commands typed at the REPL).

| Tool | Description |
|------|-------------|
| `read` | Read files with line pagination |
| `write` | Create or overwrite files |
| `edit` | Replace text in files |
| `glob` | Find files by name pattern (faster with `fd`) |
| `grep` | Search file contents (faster with `rg`) |
| `nu` | Execute a Nushell command |
| `bash` | Execute shell commands |

Optional speedups (auto-detected, falls back to pure Rust):

```bash
cargo install ripgrep fd-find
```

## File References

Reference files in prompts with `@filename` syntax — contents are injected into the conversation.

```
Refactor @crates/looprs-cli/src/main.rs for better error handling
Compare @crates/looprs/src/agent.rs and @crates/looprs/src/api.rs
```

## Extensibility

The `.looprs/` directory defines repo-local agent configuration. All extension points support dual-source loading: user-level (`~/.looprs/`) and repo-level (`.looprs/`), with repo taking precedence.

```
.looprs/
├── provider.json          # Provider/model settings
├── config.json            # Runtime defaults, file refs, pipeline, agents, paths
├── commands/              # Custom slash commands (/)
├── hooks/                 # Event-driven hooks (YAML)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
└── rules/                 # Constraints and guidelines (Markdown)
```

`config.json` is loaded into `AppConfig` and supports:

- `defaults`: runtime limits such as context tokens, temperature, and timeout.
- `file_references`: allowed `@file` reference extensions and maximum file size.
- `onboarding`: onboarding state, with `.looprs/state.json` taking precedence at runtime.
- `pipeline`: optional pipeline checks, compaction settings, and log directory.
- `agents`: delegation defaults, filesystem mode, parallelism, and orchestration strategy.
- `paths`: repo-local directories for agents, commands, hooks, rules, and skills.
- `persistence`: session store backend (`sqlite` or `fs`, default `fs`).

Provider selection and model settings are separate. Put `provider`, provider-specific `model`, `max_tokens`, and `timeout_secs` in `.looprs/provider.json`.

### Commands

Define slash commands in `.looprs/commands/<name>.yaml`:

```yaml
name: test
description: Run tests
action:
  type: shell
  command: cargo nextest run
  inject_output: true
```

Action types: `prompt` (send to LLM), `shell` (run command with Nushell),
`message` (print to console), `switch_provider`, `outsource`, `list_models`.

Repo command set evolves; run `/help` for the canonical in-session list.

### Skills

Skills follow progressive disclosure: YAML frontmatter with name/description/triggers, invoked with `$skill-name` or via keyword match. Loaded from `~/.looprs/skills/` and `.looprs/skills/`.

### Agents

YAML role definitions in `.looprs/agents/`. Agent dispatcher switches roles during a session.

### Rules

Markdown constraint files in `.looprs/rules/`. Evaluated against agent behavior.

### Hooks

YAML hooks fire on session lifecycle events. Define in `.looprs/hooks/<EventName>.yaml`:

```yaml
name: show_status
trigger: SessionStart
condition: has_tool:git
actions:
  - type: command
    command: "git log --oneline -5"
    inject_as: recent_commits
  - type: command
    command: "git status --short"
    requires_approval: true
    approval_prompt: "Inject git status into context?"
```

Events: `SessionStart`, `UserPromptSubmit`, `InferenceComplete`, `PreToolUse`, `PostToolUse`, `OnError`, `OnWarning`, `SessionEnd`, `DelegationStart`, `DelegationComplete`.

Action types: `command` (Nushell command, optional `inject_as` and `requires_approval`), `message`, `conditional`.


## Observability

looprs writes structured JSONL traces and events under `~/.looprs/observability/`
by default (centralized across projects), or `.looprs/observability/` if the
current directory has its own `.looprs/config.json` (project-scoped setup):

- `<root>/traces/*.jsonl` — turn traces
- `<root>/ui_events.jsonl` — UI/machine events

Override the root explicitly:

```bash
export LOOPRS_OBSERVABILITY_DIR="$HOME/.local/share/looprs/observability"
```

Live LLM tests are gated by:

```bash
export LOOPRS_RUN_LIVE_LLM_TESTS=1
cargo test --all-targets -- --ignored
```

## Architecture

### Workspace

The repository is a Cargo workspace:

- `crates/looprs-core/` — core API, types, ports, events, and lightweight adapters
- `crates/looprs/` — agent runtime, providers, tools, hooks, skills, plugins, configuration, and observability
- `crates/looprs-cli/` — `looprs` binary, CLI argument parsing, REPL, and runtime facade
- `crates/looprs-tui/` — `looprs provider` (provider/model select menu) and `looprs tui` (alternate chat TUI)
- `xtask/` — local automation shim that delegates to `taskit`
- `tests/` — workspace integration tests
- `fuzz/` — fuzz targets, excluded from the default workspace

See [`docs/ownership-model.md`](./docs/ownership-model.md) for canonical ownership boundaries.

## Dev

```bash
cargo build --workspace
cargo nextest run --workspace
cargo nextest run -p looprs-cli --bin looprs
cargo clippy --all-targets --all-features -- -D warnings
cargo xtask check pre-push
```

Primary quality gates run through `cargo` and `cargo xtask` (not `make`). The `Makefile` remains as a convenience layer for common shortcuts such as `make build`, `make lint`, and `make all`. Use `cargo xtask check pre-push` before pushing; the installed `.githooks/pre-push` delegates to the same command and includes the `looprs-cli` binary test suite that library-only shortcuts do not cover.

## License

MIT
