//! SessionStore port — abstraction over session event persistence.

use std::path::Path;

use serde::Serialize;

/// A discrete event that can be recorded in a session log.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A user-authored message.
    UserMessage {
        /// Message text as entered by the user.
        content: String,
        /// Provider active when the message was submitted.
        provider: String,
    },
    /// A completed model inference response.
    Inference {
        /// Assistant text/content payload.
        content: String,
        /// Provider that generated the inference.
        provider: String,
    },
    /// A tool invocation requested by the model.
    ToolUse {
        /// Tool name requested by the model.
        tool_name: String,
        /// Structured JSON input passed to the tool.
        input: serde_json::Value,
        /// Tool-use identifier from the provider payload.
        tool_use_id: String,
        /// Provider that emitted this tool call.
        provider: String,
    },
    /// The result produced by a prior tool invocation.
    ToolResult {
        /// Tool-use identifier that this result completes.
        tool_use_id: String,
        /// Tool output text.
        output: String,
        /// Whether this result represents an error path.
        is_error: bool,
        /// Provider that emitted the corresponding tool call.
        provider: String,
    },
    /// Explicit end-of-session marker.
    SessionEnd,
}

// NOTE: Hex refactor Phase 3 is complete: `Agent::new_with_runtime` accepts an
// injected `SessionStore`, and the looprs-cli composition root wires
// `default_session_store()` into the agent.
/// Port: append session events to a durable store.
///
/// Implementations decide the storage backend (filesystem JSONL, SQLite, etc.).
pub trait SessionStore: Send {
    /// Record a session event.
    fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error>;

    /// Return the canonical path associated with this session's log, if any.
    fn path(&self) -> Option<&Path>;

    /// Return the unique identifier for this session.
    fn session_id(&self) -> &str;
}
