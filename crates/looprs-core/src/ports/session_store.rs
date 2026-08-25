//! SessionStore port — abstraction over session event persistence.

use std::path::Path;

use serde::Serialize;

/// A discrete event that can be recorded in a session log.
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    UserMessage {
        content: String,
        provider: String,
    },
    Inference {
        content: String,
        provider: String,
    },
    ToolUse {
        tool_name: String,
        input: serde_json::Value,
        tool_use_id: String,
        provider: String,
    },
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
        provider: String,
    },
    SessionEnd,
}

// TODO: hex refactor Phase 3 — Agent still constructs SessionLogger
// internally rather than receiving it as an injected SessionStore. Note that
// the persistence half of this idea is done: SqliteSessionStore implements
// this trait, and ObservationManager::persist/load_from already round-trip
// through SQLite (see the TODO in observation_manager.rs for the remaining
// gap — the read path is never wired into a running command).
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
