# Agent Instructions

This document provides guidance for AI agents working on the looprs codebase.

## Project Overview

looprs is a unified abstraction layer for agentic AI that provides:
- Multi-provider LLM support (Anthropic, OpenAI, local models via Ollama)
- Built-in tools for file operations and shell execution
- Extensibility through commands, skills, agents, rules, and hooks
- Event-driven architecture with lifecycle hooks

## Development Tools

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

## Architecture Overview

### CLI Application (`src/bin/looprs/`)

The CLI is organized into focused modules:

- **`main.rs`** - Entry point with argument parsing and initialization
- **`cli.rs`** - CLI configuration and setup logic
- **`repl.rs`** - Interactive REPL loop handling user input and responses
- **`args.rs`** - Command-line argument definitions using `clap`

**Design principle:** Keep the CLI layer thin - it coordinates between user input and the core library.

### Core Library (`src/`)

The main library components:

- **`agent.rs`** - Central orchestrator managing:
  - LLM provider interactions
  - Message history and context
  - Tool execution lifecycle
  - Event firing and hook invocation
  - Observation capture

- **`app_config.rs`** - Centralized configuration management
  - Application-wide settings
  - Runtime configuration
  - Shared state coordination

- **`providers/`** - LLM provider implementations:
  - `anthropic.rs` - Claude models (native tool support)
  - `openai.rs` - GPT models (function calling)
  - `local.rs` - Ollama integration (limited tool support)
  - `mod.rs` - `LLMProvider` trait and auto-detection

- **`tools/`** - Built-in capabilities exposed to LLMs:
  - `bash.rs` - Shell command execution
  - `read.rs` - File reading with pagination
  - `write.rs` - File creation/overwriting
  - `edit.rs` - Text replacement
  - `grep.rs` - Content search (ripgrep integration)
  - `glob.rs` - File pattern matching (fd integration)

- **`events.rs` + `hooks/`** - Event-driven system:
  - 8 lifecycle events (SessionStart, SessionEnd, etc.)
  - YAML-based hook definitions in `.looprs/hooks/`
  - Actions: command execution, context injection, conditionals

- **`context.rs`** - Session context collection:
  - Auto-gathers repo status (jj), open issues (bd), board state (kan)
  - Injected into system prompts for contextual awareness

- **`observation.rs` + `observation_manager.rs`** - Incremental learning:
  - Captures tool executions across sessions
  - Stores in bd for continuity
  - Loaded on SessionStart for agent memory

### Extensibility Framework (`.looprs/`)

All customization happens in `.looprs/` without modifying core:

```
.looprs/
├── provider.json     # LLM provider settings
├── config.json       # Global configuration
├── hooks/            # Event-driven automation (YAML)
├── commands/         # Custom slash commands (/)
├── skills/           # Progressive disclosure capabilities ($)
├── agents/           # Agent role definitions
└── rules/            # Constraints and guidelines
```

## Code Style Guidelines

### Error Handling
- Use `anyhow::Result` for functions that can fail
- Provide context with `.context()` or `.with_context()`
- Graceful degradation for optional features (don't crash if jj/bd missing)

### Async/Await
- All LLM API calls are async (tokio runtime)
- Use `#[tokio::main]` in bin, `async_trait` for providers
- Tool execution is synchronous but may shell out to async processes

### Testing
- Unit tests alongside implementation in `src/`
- Integration tests in `tests/`
- Run with `make test` or `cargo test --lib`

### Module Exports
- `lib.rs` defines public API surface
- Export only what's needed externally
- Keep internal implementation details private

## Common Tasks

### Adding a New Tool

1. Create `src/tools/newtool.rs`:
```rust
use anyhow::Result;
use serde_json::{json, Value};

pub fn execute(params: Value) -> Result<Value> {
    // Implementation
    Ok(json!({"result": "success"}))
}
```

2. Register in `src/tools/mod.rs`:
```rust
pub mod newtool;
// Add to tools vector in get_tools()
```

3. Add tests in `src/tools/newtool.rs`

### Adding an Event

1. Add variant to `Event` enum in `src/events.rs`
2. Fire event in appropriate location:
```rust
self.events.fire(Event::NewEvent, &mut event_ctx);
```
3. Update hook documentation

### Modifying Provider Logic

Each provider has its own message format and tool calling convention:
- **Anthropic**: native `tool_use` blocks in content array
- **OpenAI**: `tool_calls` array + separate `tool` role messages
- **Local**: text-based markers (limited)

Be careful when changing provider logic - test all three providers.

## Quality Gates

Before committing changes:

```bash
make fmt        # Format code
make lint       # Run clippy
make test       # Run tests
make build      # Verify compilation
```

Or run all at once:
```bash
make all        # fmt-check, lint, test, build
```

## Debugging Tips

### REPL not responding
- Check provider API keys are set
- Verify network connectivity
- Look for error messages in console

### Tool execution fails
- Check tool exists in PATH (for external tools)
- Verify parameters match expected format
- Look at tool output in conversation

### Hook not firing
- Verify YAML syntax is valid
- Check event name matches exactly
- Confirm `.looprs/hooks/` directory exists
- Look for warning messages on SessionStart

