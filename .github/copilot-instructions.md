# Copilot Instructions for looprs

## Project Overview

looprs is a unified abstraction layer for agentic AI - a REPL that interfaces with LLMs (Claude, GPT, local models) and provides extensibility through commands, skills, agents, rules, hooks, and file references.

**Core concept:** LLM + tools + extensibility framework = everything needed to build agents.

## Build, Test, and Lint Commands

### Build
```bash
make build       # Release build (recommended)
make dev         # Debug build
cargo build --release
```

### Test
```bash
make test        # Run library tests
make test-all    # Run all tests including integration
cargo test --lib # Single test scope

# Run specific test
cargo test --lib test_name
cargo test --lib --package looprs test_name
```

### Lint and Format
```bash
make lint        # Clippy with warnings-as-errors
make fmt         # Format code
make fmt-check   # Check formatting without modifying
make all         # Run all checks (check, fmt-check, lint, test)
```

### Development
```bash
make watch       # Watch mode (requires bacon)
make setup       # Install dev dependencies
```

## Architecture

### Core Components

**Agent (`src/agent.rs`)**
- Central orchestrator managing LLM provider, messages, tools, events, and hooks
- Implements the main conversation loop with tool-use support
- Coordinates between provider inference and tool execution

**Providers (`src/providers/`)**
- Abstraction over multiple LLM backends via `LLMProvider` trait
- `anthropic.rs` - Claude models (Opus, Sonnet, Haiku)
  - Uses `max_tokens` parameter
  - Native tool support with structured responses
- `openai.rs` - GPT models
  - GPT-5.x and newer GPT-4 models: use `max_completion_tokens`
  - Older GPT-4 models: use `max_tokens`
  - Detection logic based on model name prefix
  - Function calling format for tools
- `local.rs` - Ollama integration
  - No max_tokens support
  - Limited tool use (text-based markers only)
- Auto-detection from environment variables or explicit configuration

**Tools (`src/tools/`)**
- Built-in capabilities exposed to the LLM
- `bash.rs` - Shell command execution
- `read.rs` - File reading with line pagination
- `write.rs` - File creation/overwriting
- `edit.rs` - Text replacement in files
- `grep.rs` - Content search (with ripgrep integration)
- `glob.rs` - File pattern matching (with fd integration)
- Performance optimization: auto-detects `rg`/`fd` for 10-100x speedup

**Event System (`src/events.rs`)**
- Event-driven architecture with 8 lifecycle events:
  - `SessionStart`, `SessionEnd`
  - `UserPromptSubmit`, `InferenceComplete`
  - `PreToolUse`, `PostToolUse`
  - `OnError`, `OnWarning`
- Hooks can subscribe to events for context injection, approval gates, automation

**Hooks (`src/hooks/`)**
- YAML-based event handlers in `.looprs/hooks/`
- Actions: execute commands, inject context, conditional logic
- Graceful degradation: missing hooks/failed execution doesn't break session

**Session Context (`src/context.rs`)**
- Automatically collected on startup from:
  - `jj` (Jujutsu) - repo status and recent commits
  - `bd` (beads.db) - open issues
- Injected into prompts for contextual awareness

**Observations (`src/observation.rs`, `src/observation_manager.rs`)**
- Incremental learning: captures tool executions across sessions
- Stored in bd for continuity between sessions

### Extensibility Framework

All extensibility lives in `.looprs/` directory:
```
.looprs/
├── provider.json        # Provider settings (persists LLM choice)
├── config.json          # Global configuration
├── hooks/               # Event-driven automation (YAML)
├── commands/            # Custom slash commands
├── skills/              # Progressive disclosure capabilities
├── agents/              # Agent role definitions
└── rules/               # Constraints and guidelines
```

Design principle: **extend without modifying core** - all customization via `.looprs/` files.

## Key Conventions

### Provider Configuration Priority
1. Environment variables (`PROVIDER`, `MODEL`, `*_API_KEY`)
2. `.looprs/provider.json`
3. Auto-detection from available API keys

### Provider API Differences
- **Token limits:** OpenAI changed from `max_tokens` to `max_completion_tokens` in GPT-5+ models
  - Detection: check if model name starts with `gpt-5`, `gpt-4o`, or `gpt-4-turbo-2024`
  - Always use the correct parameter for the model to avoid 400 errors
- **Tool calling:** Each provider has different tool call formats
  - Anthropic: native `tool_use` blocks in content
  - OpenAI: `function` objects in separate `tool_calls` array
  - Local: text-based markers (limited support)
- **System messages:** OpenAI puts system in messages array, Anthropic uses separate field

### Error Handling
- Use `anyhow::Result` for functions that can fail
- Graceful degradation for optional features (jj, bd, external tools)
- Tool execution failures should not crash the session

### Tool Execution
- Tools return `serde_json::Value` to LLM
- Output captured for observation system
- External tool detection via `which` command (`rg`, `fd`, `jj`, `bd`)

### Async Context
- All LLM API calls are async (tokio runtime)
- Tool execution is synchronous but can shell out to async processes
- Use `#[tokio::main]` in bin, `async_trait` for provider implementations

### Testing
- Unit tests in `src/` alongside implementation
- Integration tests in `tests/`
- `cli_smoke.rs` for end-to-end validation

### Rust Edition and Version
- Edition: 2024 (latest Rust edition)
- Minimum Rust version: 1.88
- Check with `make verify-rust`

### Module Structure
- `lib.rs` exports public API
- Most functionality is library code (bin is thin wrapper)
- Public exports: `Agent`, `ApiConfig`, `ProviderConfig`, `SessionContext`, `Event`, `EventContext`, `EventManager`, `Hook`, `HookExecutor`, `HookRegistry`, `Observation`, `ObservationManager`

### Hook Development
- Place YAML files in `~/.looprs/hooks/` named by event (e.g., `SessionStart.yaml`)
- Support `command`, `message`, and `conditional` action types
- Use `inject_as` to feed command output into EventContext
- Conditions: `on_branch:*`, `has_tool:name`

### Dependencies
- Prefer minimal, well-maintained crates
- Async HTTP: `reqwest` with rustls (no native-tls)
- CLI: `rustyline` for REPL
- Serialization: `serde`, `serde_json`, `serde_yaml`
- Regex support for grep tool

## Environment Setup

Required for basic usage:
```bash
# Anthropic (recommended)
export ANTHROPIC_API_KEY="sk-ant-..."

# OR OpenAI
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4-turbo"

# OR Local
ollama serve  # separate terminal
export PROVIDER="local"
```

Optional performance tools:
```bash
cargo install ripgrep   # Fast grep
cargo install fd-find   # Fast glob
```

Optional integrations:
- `jj` (Jujutsu VCS) - repo status in SessionContext
- `bd` (beads.db) - issue tracking in SessionContext

## Pre-commit Hooks

Uses `prek` for pre-commit automation:
- Runs `cargo test` and `cargo clippy` before commits
- See `.pre-commit-config.yaml` for configuration
