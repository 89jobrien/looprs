# looprs-core

The portable domain layer. Contains port traits, pure domain types, lightweight adapters,
and macros. Has no dependency on `looprs` or `looprs-cli`.

## Purpose

`looprs-core` defines *what* the application domain needs from external systems — not how
those needs are fulfilled. Concrete implementations live in `looprs::adapters` or
`looprs-cli`. This separation makes it possible to test business logic without a real LLM,
filesystem, or terminal.

## Modules

| Module | Contents |
|--------|----------|
| `ports` | Port trait definitions (see below) |
| `adapters` | Portable adapters: `ChannelBroker`, `NullOutput`, `TerminalOutput`, `FsSessionStore` |
| `api` | `Message`, `ContentBlock`, `ToolDefinition`, and related API-layer types |
| `events` | Domain event types |
| `observation` | `Observation` type for session-level telemetry |
| `types` | Newtypes: `ModelId`, `ToolId`, `ToolName` |
| `macros` | `newtype_id!` macro for typed string wrappers |

## Ports

All ports are defined in `looprs_core::ports` and re-exported from the crate root.

### `InferenceProvider`

Abstraction over LLM inference backends.

```rust
pub trait InferenceProvider: Send + Sync {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn Error + Send + Sync>>;
    fn name(&self) -> &str;
    fn model(&self) -> &ModelId;
    fn supports_tool_use(&self) -> bool;
    fn supports_streaming(&self) -> bool;
    fn validate_config(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn infer_stream(&self, req: &InferenceRequest) -> InferStream;
}
```

Implementations: `anthropic`, `openai`, `local` (Ollama), `anthropic-sdk`, `openai-sdk`,
`claude-sdk`. All implementations call `assert_inference_provider_contract` in their test
modules (via the `test-contracts` feature).

### `UserOutput`

Abstraction over user-facing terminal or UI output.

```rust
pub trait UserOutput: Send + Sync {
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
    fn assistant_text(&self, text: &str);
    fn tool_call(&self, tool_name: &str, input_preview: &str);
    fn tool_ok(&self);
    fn tool_err(&self, err_msg: &str);
    fn write_chunk(&self, chunk: &str); // default: delegates to assistant_text
}
```

**Status**: shipped. `Agent` holds an injected `output: Box<dyn UserOutput>` field
(`with_output()`), and `run_turn_streaming()` drives `write_chunk()` per chunk during
streaming inference.

Portable adapters: `NullOutput` (tests), `TerminalOutput` (terminal). `crates/looprs-tui`
implements its own `ChannelOutput` adapter (forwards events over an `mpsc` channel to
drive the `looprs tui` chat view).

### `SessionStore`

Abstraction over session event persistence.

```rust
pub trait SessionStore: Send {
    fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error>;
    fn path(&self) -> Option<&Path>;
    fn session_id(&self) -> &str;
}
```

Implementations: `SqliteSessionStore` (`~/.looprs/sessions.db`) and `FsSessionStore`
(`~/.looprs/sessions/`). Selected via `persistence.session_store` in `config.json`.

### `MessageBroker`

Fan-out pub/sub message routing.

```rust
pub trait MessageBroker: Send + Sync {
    fn publish(&self, msg: Message) -> usize;
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;
    fn close(&self);
}
```

Implementation: `ChannelBroker` (tokio broadcast channel).

### `PluginExecutor`

Abstraction over named CLI tool execution.

```rust
pub trait PluginExecutor: Send + Sync {
    fn has_tool(&self, tool: &str) -> bool;
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> io::Result<Output>;
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
```

**Status**: defined and implemented (`PluginsAdapter`), but `Agent`'s tool dispatch does
not go through this port — it uses a separate `looprs::tools::executor::ToolExecutor`
trait instead (`Agent.tool_executor: Box<dyn ToolExecutor>`, injected via
`with_tool_executor()`, default `DefaultToolExecutor`). `PluginExecutor` is used for named
external-binary calls (`doob`, `rg`, `fd`, `git` — see `looprs::plugins`), a distinct
concern from LLM tool-call dispatch.

Implementation: `PluginsAdapter` (in `looprs::adapters`).

### `ObservationStore`

Abstraction over observation persistence.

```rust
pub trait ObservationStore: Send {
    fn save(&self, observations: &[Observation]) -> Result<(), anyhow::Error>;
}
```

**Status**: implemented by the in-memory reference store and
`looprs::SqliteObservationStore`. The non-streaming `Agent::run_turn` path idempotently
persists non-empty observation batches after successful turns, and the SQLite adapter runs
the shared store contract in its tests. Streaming callers can persist through the public
observation APIs; automatic streaming persistence is not yet wired.

## Domain Types

| Type | Description |
|------|-------------|
| `ModelId` | Typed newtype for model identifiers; provides `max_tokens()` by model family |
| `ToolId` | Typed newtype for tool identifiers |
| `ToolName` | Typed newtype for tool names |
| `Message` | Pub/sub message routed through `MessageBroker` |
| `Observation` | A single session-level telemetry event |
| `InferenceRequest` | Input to `InferenceProvider::infer` |
| `InferenceResponse` | Output from `InferenceProvider::infer` |
| `Usage` | Token usage reported by the provider |
| `SessionEvent` | A discrete event persisted by `SessionStore` |

## Test Contracts

The `test-contracts` feature exports shared validators for inference providers,
observation stores, session stores, message brokers, user output, plugin supervision,
and remote model catalogs. Enable it in dev-dependencies:

```toml
looprs-core = { path = "../looprs-core", features = ["test-contracts"] }
```
