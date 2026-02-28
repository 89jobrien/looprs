# Ownership Model

This document defines ownership boundaries for `looprs` so implementation decisions stay consistent across `src/`, `crates/`, and extension directories.

## Canonical Ownership

### `src/` — Core runtime ownership

`src/` is the canonical application core. Core orchestration, runtime behavior, providers, tools, hooks execution, and shared contracts live here.

Examples:

- `src/agent.rs` (orchestration loop)
- `src/providers/*` (provider integrations)
- `src/tools/*` (tool registry and execution)
- `src/hooks/*`, `src/events.rs` (lifecycle + hook execution)
- `src/app_config.rs`, `src/context.rs`, `src/observation*`

Rule: product/runtime behavior that applies across all surfaces belongs in `src/`.

### `crates/` — App surfaces and adapters

`crates/` contains app-specific frontends and adapters that consume core behavior from `src/`.

For this repository, `crates/looprs-desktop` is the desktop application surface and should own desktop-only UI/modules.

Rule: platform/surface concerns belong in the owning crate; do not move shared orchestration out of `src/`.

### `.looprs/` and `~/.looprs/` — Extension/config ownership

`.looprs/` (repo) and `~/.looprs/` (user) are extension and configuration surfaces.

- Hooks, commands, skills, agents, and rules are configured here.
- Repo entries override user entries when names collide.
- Normal application flow must not silently overwrite user-controlled config.

Rule: `.looprs/` is for customization; core logic changes must not be implemented by mutating user config as a side effect.

## Boundary Rules

1. Keep CLI thin (`src/bin/looprs/*`) and route core logic to `src/` modules.
2. Keep cross-surface contracts in `src/`; keep surface presentation/runtime wrappers in `crates/*`.
3. Treat extension directories as policy/config input, not a place for hidden core state mutations.
4. Integrate external code through adapters; do not replace core orchestration with vendored subsystems.

## Practical PR Checklist

- Does this change alter shared runtime behavior? → implement in `src/`.
- Does this change only affect desktop UI/surface concerns? → implement in `crates/looprs-desktop`.
- Does this change require user customization? → implement through `.looprs/` resources, preserving precedence rules.
- Does this write user config during normal operation? → avoid unless explicitly documented as user-controlled/opt-in.
