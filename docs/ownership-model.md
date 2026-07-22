# Ownership Model

This document defines ownership boundaries for `looprs` so implementation decisions stay consistent across the workspace crates and extension directories.

## Canonical Ownership

### `crates/looprs-core/` — Shared domain layer

`looprs-core` is the portable foundation: pure types, port traits, events, and lightweight adapters that carry no dependency on `looprs` internals.

Examples:

- `crates/looprs-core/src/ai_types.rs` (shared AI message/content types)
- `crates/looprs-core/src/ports/` (trait abstractions: `InferenceProvider`, `MessageBroker`, `SessionStore`, `ObservationStore`, `UserOutput`, `PluginExecutor`)
- `crates/looprs-core/src/events.rs` (lifecycle event definitions and `EventManager`)
- `crates/looprs-core/src/adapters/` (portable adapters: `FsSessionStore`, `ChannelBroker`, `NullOutput`, `TerminalOutput`)

Rule: Changes to shared contracts in `looprs-core` affect all dependent crates (`looprs`, `looprs-cli`). Update protocol surfaces tracked in `taskit-protocol.lock` when modifying `api.rs` or `types.rs`.

### `crates/looprs/` — Core runtime

`crates/looprs/` is the canonical application runtime. Core orchestration, providers, tools, hooks execution, and shared configuration logic live here.

Examples:

- `crates/looprs/src/agent.rs` (orchestration loop)
- `crates/looprs/src/providers/` (Anthropic, Anthropic SDK, OpenAI, OpenAI SDK, Local/Ollama)
- `crates/looprs/src/tools/` (tool registry and execution)
- `crates/looprs/src/hooks/`, `crates/looprs/src/events.rs` (lifecycle + hook execution)
- `crates/looprs/src/app_config.rs`, `crates/looprs/src/state.rs`, `crates/looprs/src/context.rs`
- `crates/looprs/src/adapters/` (infrastructure adapters: `SqliteSessionStore`, `RetryProvider`, `PluginsAdapter`)

Rule: Product/runtime behavior that applies across all surfaces belongs in `crates/looprs/`.

### `crates/looprs-cli/` — CLI surface

`crates/looprs-cli/` is the interactive terminal surface. It owns argument parsing, the REPL loop, key bindings, and the `looprs` binary.

Examples:

- `crates/looprs-cli/src/main.rs` (entry point)
- `crates/looprs-cli/src/args.rs` (CLI argument parsing)
- `crates/looprs-cli/src/repl.rs` (REPL key bindings and completion)
- `crates/looprs-cli/src/runtime/` (runtime facade and session wiring)

Rule: Keep the CLI thin — coordinate between user input and `crates/looprs/` modules. Do not implement shared orchestration in `looprs-cli`.

### `.looprs/` and `~/.looprs/` — Extension/config ownership

`.looprs/` (repo) and `~/.looprs/` (user) are extension and configuration surfaces.

- Hooks, commands, skills, agents, and rules are configured here.
- Repo entries override user entries when names collide.
- `config.json` and `provider.json` are **user-owned** — normal application flow must not overwrite them.
- `state.json` is **app-managed** (onboarding flags, etc.) and may be written by the app.
- Use `looprs seed [DIR]` to generate example config files.

Rule: `.looprs/` is for customization; core logic changes must not be implemented by mutating user config as a side effect.

## Boundary Rules

1. Keep the CLI thin (`crates/looprs-cli/src/`) and route core logic to `crates/looprs/` modules.
2. Keep shared domain types and port traits in `crates/looprs-core/`; keep runtime implementation in `crates/looprs/`.
3. Treat extension directories as policy/config input, not a place for hidden core state mutations.
4. Integrate external code through adapters; do not replace core orchestration with vendored subsystems.

## Practical PR Checklist

- Does this change alter shared runtime behavior? → implement in `crates/looprs/`.
- Does this change shared types or port traits? → implement in `crates/looprs-core/` and update `taskit-protocol.lock`.
- Does this change only affect the CLI/REPL? → implement in `crates/looprs-cli/`.
- Does this change require user customization? → implement through `.looprs/` resources, preserving precedence rules.
- Does this write user config (`config.json`, `provider.json`) during normal operation? → avoid; use `state.json` for app-managed flags.
