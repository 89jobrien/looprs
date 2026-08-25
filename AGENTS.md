# Agent Instructions

This document provides guidance for AI agents working on the looprs codebase.

## Project Overview

looprs is a Rust LLM agent loop CLI and library — a full agent runtime:
multi-turn conversation management, tool execution, provider abstraction, and
a YAML-based extension system for hooks, skills, commands, agents, and rules.

## Development Tools

Issue tracking, planning, and cross-repo work for this project happen through
the `doob` CLI and Linear (see the workspace-level `~/dev/CLAUDE.md` for
Linear team/workspace details) — not a repo-local issue tracker. Quality
gates run through `cargo` / `cargo xtask` (which delegates to `taskit`), not
`make`.

## Quick Reference

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo xtask check pre-push    # required before pushing; runs fmt + clippy + CLI binary tests
```

## Landing the Plane (Session Completion)

**When ending a work session**, work is NOT complete until `git push`
succeeds (only push when explicitly asked to — see `~/.claude/CLAUDE.md`).

**MANDATORY WORKFLOW:**

1. **Run quality gates** (if code changed) — `cargo fmt`, `cargo clippy`, `cargo nextest run --workspace`, `cargo xtask check pre-push`
2. **File follow-up work** — `doob todo add` for anything that needs a later pass
3. **Commit** — only when explicitly asked
4. **Push** — only when explicitly asked; verify with `git status` afterward
5. **Hand off** — update `.ctx/HANDOFF.<repo>.<repo>.yaml` per the workspace convention

## Architecture Overview

### Workspace layout

```
crates/looprs-core/   — core API types, port traits, lightweight adapters
crates/looprs/        — agent runtime, providers, tools, hooks, skills, plugins, config
crates/looprs-cli/    — looprs binary, CLI arg parsing, REPL, runtime facade
crates/looprs-tui/    — `looprs provider` (select menu) and `looprs tui` (chat TUI)
xtask/                — local automation shim (delegates to taskit)
tests/                — workspace integration tests
fuzz/                 — fuzz targets (excluded from default workspace)
```

**Ownership rules** are canonical in `docs/ownership-model.md`:
- Shared runtime behavior → `crates/looprs/`
- Port traits and domain types → `crates/looprs-core/`
- CLI/surface concerns only → `crates/looprs-cli/`
- Interactive terminal UI → `crates/looprs-tui/`
- Customization/config → `.looprs/` (never mutate as a side effect of normal operation)

### Core Library (`crates/looprs/src/`)

- **`agent.rs`** — Central orchestrator managing:
  - LLM provider interactions
  - Message history and context
  - Tool execution lifecycle
  - Event firing and hook invocation
  - Observation capture

- **`app_config.rs`** — Centralized configuration management (`.looprs/config.json` → `AppConfig`)

- **`providers/`** — LLM provider implementations, selected via `PROVIDER` env var or `.looprs/provider.json`:
  - `anthropic.rs` — Claude models (native tool support)
  - `openai.rs` — GPT models (function calling)
  - `local.rs` — Ollama integration
  - `gemini.rs` — Google Gemini (`gemini`, alias `google`)
  - `baml_provider.rs` — BAML-backed provider (`baml`)
  - `anthropic_sdk.rs` / `openai_sdk.rs` — SDK-backed variants (`anthropic-sdk`, `openai-sdk`, `claude-sdk`)
  - `mod.rs` — `LLMProvider` trait and provider selection

- **`tools/`** — Built-in capabilities exposed to LLMs: `bash.rs`, `read.rs`,
  `write.rs`, `edit.rs`, `grep.rs` (ripgrep integration), `glob.rs` (fd
  integration), `nu.rs` (Nushell), plus `executor.rs`/`error.rs`

- **`events.rs` + `hooks/`** — Event-driven system:
  - 8 lifecycle events: `SessionStart`, `UserPromptSubmit`, `InferenceComplete`,
    `PreToolUse`, `PostToolUse`, `OnError`, `OnWarning`, `SessionEnd`
  - YAML-based hook definitions in `.looprs/hooks/`
  - Actions: command execution (via Nushell), context injection, conditionals

- **`context.rs`** — Session-start context collection: git repo status
  (`git_info.rs`) and pending `doob` todos scoped to this project
  (`doob.rs`), injected into the system prompt

- **`observability.rs`** — Structured JSONL traces/events. Root resolves to
  `LOOPRS_OBSERVABILITY_DIR` if set, else `.looprs/observability/` if the
  cwd has its own `.looprs/config.json` (project-scoped), else
  `~/.looprs/observability/` (centralized default)

- **`observation.rs` + `observation_manager.rs`** — Incremental learning:
  captures tool executions for context injection across turns

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

`.looprs/` (repo-level) overrides `~/.looprs/` (user-level) on name collision.

## Code Style Guidelines

### Error Handling
- Use `anyhow::Result` for functions that can fail
- Provide context with `.context()` or `.with_context()`
- No `unwrap()`/`expect()` in production paths
- Graceful degradation for optional features (don't crash if `doob`/`ollama` missing)

### Async/Await
- All LLM API calls are async (tokio runtime)
- Use `#[tokio::main]` in bin, `async_trait` for providers
- Tool execution is synchronous but may shell out to async processes

### Testing
- `cargo nextest run --workspace`, not `cargo test`
- Unit tests alongside implementation in `src/`
- Integration tests in `tests/`
- Live LLM tests are off by default — enable with `LOOPRS_RUN_LIVE_LLM_TESTS=1`

### Module Exports
- `lib.rs` defines public API surface
- Export only what's needed externally
- Directory `mod.rs` layout, not flat files

## Common Tasks

### Adding a New Tool

1. Create `crates/looprs/src/tools/newtool.rs`
2. Register it in `crates/looprs/src/tools/mod.rs`
3. Add tests alongside the implementation

### Adding an Event

1. Add variant to `Event` enum in `crates/looprs/src/events.rs`
2. Fire event in the appropriate location:
```rust
self.fire_event(Event::NewEvent, &event_ctx);
```
3. Update hook documentation

### Modifying Provider Logic

Each provider has its own message format and tool calling convention:
- **Anthropic**: native `tool_use` blocks in content array
- **OpenAI**: `tool_calls` array + separate `tool` role messages
- **Local**: text-based markers (limited tool support)

Be careful when changing provider logic — test all providers, or at minimum
run against `local`/Ollama for a real end-to-end check (see
`.claude/skills/run-looprs/SKILL.md`).

## Quality Gates

Before committing changes:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo build --workspace
```

Or the pre-push gate used by `.githooks/pre-push`:

```bash
cargo xtask check pre-push
```

## Debugging Tips

### REPL not responding
- Check provider API keys are set (or `PROVIDER=local` with `ollama serve` running)
- Verify network connectivity
- Look for error messages in console

### Tool execution fails
- Check tool exists in PATH (for external tools like `rg`/`fd`)
- Verify parameters match expected format
- Look at tool output in conversation

### Hook not firing
- Verify YAML syntax is valid
- Check event name matches exactly (see the 8 lifecycle events above)
- Confirm `.looprs/hooks/` directory exists
- Look for warning messages on `SessionStart`
