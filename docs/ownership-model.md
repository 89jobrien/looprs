# Ownership Model

This document defines ownership boundaries for `looprs` so implementation decisions stay consistent across the workspace crates and extension directories.

## Canonical Ownership

### `crates/looprs-core/` — Ports and domain types

`looprs-core` defines the port traits (e.g. `InferenceProvider`), core API
types, events, and lightweight adapters that don't need the full runtime.
It has no dependency on `crates/looprs`.

Rule: trait/type contracts shared across adapters belong here, not in `looprs`.

### `crates/looprs/` — Core runtime ownership

`crates/looprs` is the canonical application core. Orchestration, runtime
behavior, providers, tools, hooks execution, and shared contracts live here.

Examples:

- `agent.rs` (orchestration loop)
- `providers/*` (provider integrations: anthropic, openai, local, gemini, baml, SDK variants)
- `tools/*` (tool registry and execution)
- `hooks/*`, `events.rs` (lifecycle + hook execution)
- `app_config.rs`, `context.rs`, `observability.rs`, `observation*`

Rule: product/runtime behavior that applies across all surfaces belongs in `crates/looprs`.

### `crates/looprs-cli/` and `crates/looprs-tui/` — App surfaces

These crates are the presentation surfaces that consume core behavior from
`crates/looprs`. `looprs-cli` owns the REPL, CLI argument parsing, and the
`looprs` binary entrypoint. `looprs-tui` owns the interactive terminal
surfaces (`looprs provider` select menu, `looprs tui` chat view) built on
`ratatui`/`crossterm`.

Rule: platform/surface concerns belong in the owning crate; do not move
shared orchestration out of `crates/looprs`.

### `.looprs/` and `~/.looprs/` — Extension/config ownership

`.looprs/` (repo) and `~/.looprs/` (user) are extension and configuration
surfaces.

- Hooks, commands, skills, agents, and rules are configured here.
- Repo entries override user entries when names collide.
- Normal application flow must not silently overwrite user-controlled config.

Rule: `.looprs/` is for customization; core logic changes must not be
implemented by mutating user config as a side effect.

## Boundary Rules

1. Keep CLI thin (`crates/looprs-cli/src/`) and route core logic to `crates/looprs` modules.
2. Keep cross-surface contracts in `crates/looprs-core`; keep surface presentation/runtime wrappers in `crates/looprs-cli` and `crates/looprs-tui`.
3. Treat extension directories as policy/config input, not a place for hidden core state mutations.
4. Integrate external code (git, doob, ollama, rg, fd) through the `plugins/` adapter layer; do not replace core orchestration with vendored subsystems.

## Practical PR Checklist

- Does this change alter shared runtime behavior? → implement in `crates/looprs`.
- Does this change only affect CLI/REPL presentation? → implement in `crates/looprs-cli`.
- Does this change only affect the interactive TUI (provider menu / chat view)? → implement in `crates/looprs-tui`.
- Does this change belong in a port trait shared across adapters? → implement in `crates/looprs-core`.
- Does this change require user customization? → implement through `.looprs/` resources, preserving precedence rules.
- Does this write user config during normal operation? → avoid unless explicitly documented as user-controlled/opt-in.
