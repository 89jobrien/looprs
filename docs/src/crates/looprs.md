# looprs

The agent runtime crate. Owns the inference loop, provider implementations, tool
execution, extension loading, configuration, and all infrastructure adapters that depend
on looprs internals.

## Purpose

`looprs` is where things happen at runtime: the `Agent` struct drives multi-turn
conversation, providers translate to LLM APIs, tools execute on the local system, and
the extension system loads user-defined hooks, skills, commands, agents, and rules.

## Public API

Key exports from `looprs::`:

| Export | Description |
|--------|-------------|
| `Agent` | The agent loop; call `run_turn()` or `run_turn_streaming()` per conversation turn |
| `ChatMessage` | Flattened (role, text) transcript entry; `Agent::transcript()` returns the full history for UI consumption |
| `RuntimeSettings` | Configuration bundle injected into `Agent::new()` |
| `SessionContext` | Session-start context (git status + pending `doob` todos), injected into the system prompt |
| `AgentRegistry` / `AgentDefinition` | Named agent role definitions loaded from `agents/` |
| `CommandRegistry` / `Command` / `CommandAction` | Slash command definitions |
| `HookRegistry` / `Hook` / `HookExecutor` | Lifecycle hook definitions and executor |
| `SkillRegistry` | Progressive-disclosure skill definitions |
| `RuleRegistry` / `Rule` | Constraint guidelines injected as context |
| `AgentError` / `ProviderError` / `ToolContextError` | Error types (thiserror + miette) |
| `RetryProvider` | Wrapping adapter: adds retry/backoff to any `InferenceProvider` |
| `SqliteSessionStore` | `SessionStore` backed by `~/.looprs/sessions.db` |
| `ChannelBroker` | `MessageBroker` backed by tokio broadcast channel |
| `NullOutput` | `UserOutput` that discards all output (for tests) |
| `PluginsAdapter` | `PluginExecutor` that shells out to CLI tools |

## Agent Loop

`Agent::run_turn()` (and its streaming sibling `run_turn_streaming()`) is the core of the
runtime. Current control flow:

1. Resolve file references (`@filename` syntax) in the user prompt
2. Inject rules from `RuleRegistry` into the system prompt
3. Call `InferenceProvider::infer()` (or `infer_stream()`) with the current message history
4. Process the response:
   - Text content → the injected `output: Box<dyn UserOutput>` port (`assistant_text()`/`write_chunk()`)
   - Tool use → the injected `tool_executor: Box<dyn ToolExecutor>` port (default `DefaultToolExecutor`)
5. Record the turn via the injected `session_logger: Option<Box<dyn SessionStore>>` port
6. Fire hook events (`InferenceComplete`, `PostToolUse`, etc.)
7. Apply context compaction (sliding window) if the context exceeds the configured limit

All three ports are injected via builder methods (`with_output()`, `with_tool_executor()`)
on top of `Agent::new_with_runtime()`, with sane defaults for callers that don't need to
override them — see `docs/hexagonal-refactor.md` for how this evolved from the original
phased plan.

## Providers

All providers implement `looprs_core::ports::InferenceProvider`. The provider is selected
at startup via `PROVIDER` env var or `.looprs/provider.json`.

| Provider name | Implementation | Notes |
|---------------|---------------|-------|
| `anthropic` | Raw HTTP via `reqwest` | Default |
| `openai` | Raw HTTP via `reqwest` | Requires `OPENAI_API_KEY` |
| `local` | Ollama HTTP API | `PROVIDER=local`, Ollama must be running |
| `gemini` / `google` | Raw HTTP via `reqwest` | Requires `GEMINI_API_KEY` |
| `baml` | `baml_provider.rs` | BAML-backed provider |
| `anthropic-sdk` | `claudius` crate | Richer SDK features |
| `openai-sdk` | `async-openai` crate | Full OpenAI client |
| `claude-sdk` | `claudius` crate | SDK variant |

All providers are wrapped by `RetryProvider` when `retry.*` is configured in `config.json`.

## Adapters

Adapters implement port traits and are the only place where infrastructure details (SQLite,
filesystem, processes) live.

| Adapter | Port | Location |
|---------|------|----------|
| `PluginsAdapter` | `PluginExecutor` | `adapters/plugin_executor.rs` |
| `McpToolExecutor` | `tools::executor::ToolExecutor` | `adapters/mcp_executor.rs` — routes agent tool calls to an MCP server |
| `RetryProvider` | `InferenceProvider` (wrapping) | `adapters/retry_provider.rs` |
| `SqliteSessionStore` | `SessionStore` | `adapters/sqlite_session_store.rs` |
| `UiOutput` | `UserOutput` | `adapters/ui_output.rs` |
| `ChannelBroker` | `MessageBroker` | re-exported from `looprs-core` |
| `NullOutput` | `UserOutput` | re-exported from `looprs-core` |
| `TerminalOutput` | `UserOutput` | re-exported from `looprs-core` |
| `FsSessionStore` | `SessionStore` | re-exported from `looprs-core` |

`default_session_store()` in `adapters/mod.rs` reads `persistence.session_store` from
`AppConfig` and returns the appropriate boxed adapter.

## Extension System

Extensions load from `.looprs/` (repo-level) and `~/.looprs/` (user-level). Repo-level
wins on name collision. The CLI composition root loads and injects the registries required
by the selected runtime mode; `Agent` itself does not discover every extension registry.

### Hook Events

Hooks fire on these lifecycle events:

| Event | When |
|-------|------|
| `SessionStart` | Before the first turn |
| `UserPromptSubmit` | After the user submits a prompt |
| `InferenceComplete` | After the LLM responds |
| `PreToolUse` | Before a tool is executed |
| `PostToolUse` | After a tool completes |
| `OnError` | When an error is raised |
| `OnWarning` | When a warning is issued |
| `SessionEnd` | After the last turn |

### Built-in Tools

| Tool name | Description |
|-----------|-------------|
| `read` | Read files with line pagination |
| `write` | Create or overwrite files |
| `edit` | Replace text in files |
| `glob` | Find files by name pattern (uses `fd` if available) |
| `grep` | Search file contents (uses `rg` if available) |
| `nu` | Execute a Nushell command |
| `bash` | Execute a shell command |

## Configuration

Runtime config is loaded by `AppConfig` from `.looprs/config.json` and
`~/.looprs/config.json`. Provider config is loaded separately from
`.looprs/provider.json`.

See the [Configuration reference](../reference/configuration.md) for all options.

## Error Types

All error types use `thiserror` for structured variants and `miette` for rich diagnostic
output in the REPL.

| Type | Used for |
|------|---------|
| `AgentError` | Agent loop failures (provider error, tool error, context limit) |
| `ProviderError` | LLM API failures (auth, rate limit, timeout, model not found) |
| `ToolContextError` | Tool execution failures (file not found, permission denied, etc.) |
