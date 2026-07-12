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

The `.looprs/` directory defines agent configuration. All extension points support dual-source loading: user-level (`~/.looprs/`) and repo-level (`.looprs/`), with repo taking precedence.

```
.looprs/
├── provider.json          # Provider settings (user-owned)
├── config.json            # Global config (user-owned, app never overwrites)
├── state.json             # App-managed flags (e.g. onboarding; written by app)
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

Action types: `prompt` (send to LLM), `shell` (run command with Nushell), `message` (print to console).

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

Events: `SessionStart`, `UserPromptSubmit`, `InferenceComplete`, `PreToolUse`, `PostToolUse`, `OnError`, `OnWarning`, `SessionEnd`, `DelegationStart`, `DelegationComplete`.

Action types: `command` (shell command, optional `inject_as` and `requires_approval`), `message`, `conditional`, `confirm`, `prompt`, `secret_prompt`, `set_env`, `set_config`.

Example using `confirm` and `set_env`:

```yaml
name: set_api_key
trigger: SessionStart
actions:
  - type: secret_prompt
    prompt: "Enter API key (leave blank to skip):"
    set_key: api_key
  - type: set_env
    name: MY_API_KEY
    from_key: api_key
```


## Observability

looprs writes structured JSONL traces and events:

- `.looprs/observability/traces/*.jsonl` — turn traces
- `.looprs/observability/ui_events.jsonl` — UI/machine events

Redirect to an external path:

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
cargo xtask pre-push
```

The `Makefile` is still available for common shortcuts such as `make build`, `make lint`, and `make all`. Use `cargo xtask pre-push` before pushing; the installed `.githooks/pre-push` delegates to the same command and includes the `looprs-cli` binary test suite that library-only shortcuts do not cover.

## License

MIT