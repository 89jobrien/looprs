# looprs

A Rust LLM agent loop CLI. Tools, loops, and conditions — no convoluted markdown parsing system.

## Install

```bash
git clone https://github.com/89jobrien/looprs.git
cd looprs
cargo build --release
./target/release/looprs
# or: cargo install --path .
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

# SDK-backed providers
export PROVIDER="openai-sdk"      # openai-sdk | anthropic-sdk | claude-sdk
looprs
```

Persistent config: `.looprs/provider.json`. All env options: `.env.example`.

## Built-in Tools

| Tool | Description |
|------|-------------|
| `/read` | Read files with line pagination |
| `/write` | Create or overwrite files |
| `/edit` | Replace text in files |
| `/glob` | Find files by name pattern (faster with `fd`) |
| `/grep` | Search file contents (faster with `rg`) |
| `/bash` | Execute shell commands |

Optional speedups (auto-detected, falls back to pure Rust):

```bash
cargo install ripgrep fd-find
```

## File References

Reference files in prompts with `@filename` syntax — contents are injected into the conversation.

```
Refactor @src/main.rs for better error handling
Compare @file1.rs and @file2.rs
```

## Extensibility

The `.looprs/` directory defines agent configuration. All extension points support dual-source loading: user-level (`~/.looprs/`) and repo-level (`.looprs/`), with repo taking precedence.

```
.looprs/
├── provider.json          # Provider settings
├── config.json            # Global config
├── commands/              # Custom slash commands (/)
├── hooks/                 # Event-driven hooks (YAML)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
└── rules/                 # Constraints and guidelines (Markdown)
```

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

Action types: `prompt` (send to LLM), `shell` (run command), `message` (print to console).

Built-in repo commands: `/help`, `/refactor`, `/test`, `/lint`.

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
condition: has_tool:jj
actions:
  - type: command
    command: "jj log -r 'main::' | head -3"
    inject_as: recent_commits
  - type: command
    command: "git status --short"
    requires_approval: true
    approval_prompt: "Inject git status into context?"
```

Events: `SessionStart`, `UserPromptSubmit`, `InferenceComplete`, `PreToolUse`, `PostToolUse`, `OnError`, `OnWarning`, `SessionEnd`.

Action types: `command` (shell, optional `inject_as` and `requires_approval`), `message`, `conditional`.

## Desktop UI

The desktop UI lives in `crates/looprs-desktop`. Built with Freya.

```bash
cargo run -p looprs-desktop
# or with mise:
mise run ui
```

### Generative UI (BAML)

The desktop includes a live Generative UI screen backed by a BAML client:

- Schema: `crates/looprs-desktop-baml-client/baml_src/generative_ui.baml`
- Generators: `crates/looprs-desktop-baml-client/baml_src/generators.baml`

To regenerate the client after editing `.baml` files:

```bash
baml-cli generate --from crates/looprs-desktop-baml-client/baml_src
```

Requires `OPENAI_API_KEY`.

## Observability

looprs writes structured JSONL traces and events:

- `.looprs/observability/traces/*.jsonl` — turn traces
- `.looprs/observability/ui_events.jsonl` — UI/machine events

Redirect to an external path:

```bash
export LOOPRS_OBSERVABILITY_DIR="/Volumes/YourSSD/looprs-observability"
```

Live LLM tests are gated by:

```bash
export LOOPRS_RUN_LIVE_LLM_TESTS=1
cargo test --all-targets -- --ignored
```

## Architecture

### Core Modules

- `src/bin/looprs/` — CLI entry point (`main.rs`, `cli.rs`, `repl.rs`, `args.rs`)
- `src/agent.rs` — Core orchestrator (messages, tools, events, hooks, observations)
- `src/app_config.rs` — Centralized configuration
- `src/providers/` — LLM backends: Anthropic, OpenAI, local (Ollama), SDK variants
- `src/tools/` — Built-in tools (read, write, edit, glob, grep, bash)
- `src/events.rs` + `src/hooks/` — Event system and hook execution
- `src/commands.rs` + `.looprs/commands/` — Command registry
- `src/skills/` — Skill loader and parser
- `src/context.rs` — SessionContext (repo state collected at startup)
- `src/pipeline/` — Context compaction and logging pipeline
- `src/plugins/` — Plugin registry and runner
- `crates/looprs-desktop/` — Freya-based desktop UI
- `crates/looprs-desktop-baml-client/` — Generated BAML client for generative UI

See [`docs/ownership-model.md`](./docs/ownership-model.md) for canonical ownership boundaries.

## Dev

```bash
make build      # build release binary
make test       # run tests
make lint       # run clippy
make install    # install locally
```

Patch versions increment automatically on push via pre-push hook (bumps `Cargo.toml`, updates `CHANGELOG.md`).

## License

MIT