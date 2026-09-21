# Architecture Overview

looprs is a five-crate Rust workspace built on hexagonal architecture. The core domain
is isolated in `looprs-core`; infrastructure adapters and the agent runtime live in
`looprs`; the CLI binary lives in `looprs-cli`; interactive terminal UI surfaces live in
`looprs-tui`.

## Crate Dependency Graph

```text
looprs-cli
    ├── looprs
    │       ├── looprs-core
    │       │       └── looprs-macros
    │       └── looprs-macros
    └── looprs-tui
            └── looprs
                    └── looprs-core
                            └── looprs-macros
```

`looprs-cli` depends on `looprs` for the agent runtime and `looprs-tui` for the
`provider`/`tui` subcommands. `looprs-tui` depends on `looprs` for `Agent`.
`looprs` depends on `looprs-core` for port traits, while `looprs-core` and `looprs`
consume the shared proc macros. No crate depends upward.

## Hexagonal Architecture

The project follows ports-and-adapters (hexagonal) architecture. The goal is for the
agent loop to depend only on port traits, with all infrastructure injected.

```text
                    ┌─────────────────────────┐
                    │       looprs-core        │
                    │   ports (trait defs)     │
                    │   domain types           │
                    └────────────┬────────────┘
                                 │ impl
               ┌─────────────────┴──────────────────┐
               │              looprs                 │
               │  Agent (runtime loop)               │
               │  Providers (InferenceProvider)      │
               │  Adapters (concrete port impls)     │
               │  Extensions (hooks, skills, cmds)   │
               └─────────────────┬──────────────────┘
                                 │ uses
               ┌─────────────────┴──────────────────┐
               │            looprs-cli               │
               │  CliArgs parser                     │
               │  REPL (rustyline)                   │
               │  Runtime facade / entrypoint        │
               └────────────────────────────────────┘
```

### Port Status

| Port | Defined in | Status |
|------|-----------|--------|
| `InferenceProvider` | `looprs-core::ports` | Wired — all providers implement it |
| `UserOutput` | `looprs-core::ports` | Wired — injected into `Agent` (`output` field, `with_output()`) |
| `ToolExecutor`* | `looprs::tools::executor` | Wired — injected into `Agent` (`tool_executor` field, `with_tool_executor()`) |
| `SessionStore` | `looprs-core::ports` | Wired — injected into `Agent` (`session_logger: Option<...>`) |
| `PluginExecutor` | `looprs-core::ports` | Defined and implemented (`PluginsAdapter`), but not consumed by `Agent` — used for named external-binary calls (`doob`/`rg`/`fd`/`git`), a separate concern from tool-call dispatch |
| `MessageBroker` | `looprs-core::ports` | Defined — `ChannelBroker` adapter wired, not consumed by `Agent::run_turn` |
| `ObservationStore` | `looprs-core::ports` | Partially wired — `SqliteObservationStore` auto-persists successful non-streaming turns and supports replay/query APIs; streaming auto-persistence is pending |

\* `ToolExecutor` here is a `looprs`-local trait (`tools/executor.rs`), distinct from
`looprs-core`'s `PluginExecutor` — see `docs/hexagonal-refactor.md`'s status note for why
the original plan's "route tool dispatch through `PluginExecutor`" didn't happen as
written.

The hexagonal refactor plan (with its actual outcome vs. original plan) is in
`docs/hexagonal-refactor.md`. `Agent::run_turn`/`run_turn_streaming` depend on injected
`UserOutput`, `ToolExecutor`, and `SessionStore` traits; `AgentServices` consolidation
(the original phase 4) was superseded by builder-method injection.

## Data Flow: Inference Loop

```text
User prompt
  → Agent::add_user_message
  → InferenceProvider::infer / infer_stream (provider trait)
    → [tool call] tool_executor.execute() (injected ToolExecutor port)
    → [output] output.assistant_text() / output.write_chunk() (injected UserOutput port)
  → message history updated
  → session_logger.log_event() (injected SessionStore port, if present)
```

## Extension System

The `.looprs/` directory defines repo-local configuration. User-level defaults come from
`~/.looprs/`. Repo-level takes precedence on name collision.

| Directory | Contents | Loaded into |
|-----------|----------|-------------|
| `commands/` | Slash command definitions (YAML) | `CommandRegistry` |
| `hooks/` | Lifecycle hooks (YAML) | `HookRegistry` |
| `skills/` | Skill definitions (YAML) | `SkillRegistry` |
| `agents/` | Agent role definitions (YAML) | `AgentRegistry` |
| `rules/` | Constraint guidelines (Markdown) | `RuleRegistry` |

Hook events: `SessionStart`, `UserPromptSubmit`, `InferenceComplete`, `PreToolUse`,
`PostToolUse`, `OnError`, `OnWarning`, `SessionEnd`, `DelegationStart`, and
`DelegationComplete`. Explicit and automatic delegated turns emit the start event, while
successfully completed delegated turns emit the completion event.

## Ownership Rules

These rules are canonical (see also `CLAUDE.md`):

- Shared runtime behaviour → `crates/looprs/`
- Port traits and domain types → `crates/looprs-core/`
- CLI/surface concerns only → `crates/looprs-cli/`
- Interactive terminal UI → `crates/looprs-tui/`
- Customisation and config → `.looprs/` (never mutated at runtime)

## Key Files

| File | Role |
|------|------|
| `crates/looprs/src/agent.rs` | Agent loop — highest-churn file |
| `crates/looprs/src/providers/` | Provider implementations |
| `crates/looprs/src/adapters/mod.rs` | Adapter registry and `default_session_store()` |
| `crates/looprs-core/src/ports/` | All port trait definitions |
| `crates/looprs-cli/src/args.rs` | CLI argument parsing |
| `crates/looprs-cli/src/repl.rs` | Interactive REPL (rustyline) |
