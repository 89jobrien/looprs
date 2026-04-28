//! Ports (hexagonal architecture) — outbound interfaces to external systems.
//!
//! Ports define what the application domain needs from external infrastructure,
//! not how those needs are fulfilled. Adapters (in impl/) provide concrete implementations.

use std::ffi::OsString;
use std::process::Output;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ── Domain type ──────────────────────────────────────────────────────────────

/// A message routed through the pub/sub broker.
///
/// `payload` is an untyped JSON value so the broker can fan-out without
/// deserializing into concrete types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Originating component or subsystem name.
    pub source: String,
    /// Wall-clock time the message was created.
    pub timestamp: DateTime<Utc>,
    /// Logical topic string (e.g. `"agent.output"`, `"tool.result"`).
    pub topic: String,
    /// Schema version of the payload — increment when the shape changes.
    pub schema_version: u32,
    /// Unstructured payload; consumers are responsible for deserialization.
    pub payload: serde_json::Value,
}

impl Message {
    /// Convenience constructor using the current UTC time.
    pub fn new(
        source: impl Into<String>,
        topic: impl Into<String>,
        schema_version: u32,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            source: source.into(),
            timestamp: Utc::now(),
            topic: topic.into(),
            schema_version,
            payload,
        }
    }
}

// ── Port ─────────────────────────────────────────────────────────────────────

/// Port: fan-out message broker for inter-component pub/sub.
///
/// Implementations must be cheaply cloneable (`Arc`-backed) so callers
/// can hold a handle without worrying about lifetimes.
///
/// # Drop-on-lag policy
///
/// When a subscriber's receive buffer is full the broker **drops the message**
/// rather than blocking the publisher. Subscribers that cannot keep up will
/// miss messages — they must handle `RecvError::Lagged` from the channel.
pub trait MessageBroker: Send + Sync {
    /// Publish `msg` to all current subscribers of `msg.topic`.
    ///
    /// Returns the number of active receivers that received the message.
    /// Returns 0 if there are no subscribers (not an error).
    fn publish(&self, msg: Message) -> usize;

    /// Subscribe to all messages on `topic`.
    ///
    /// Each call returns an independent `Receiver`. The receiver buffer
    /// holds up to 64 messages before lagging.
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;

    /// Close the broker, dropping all sender handles.
    ///
    /// Outstanding receivers will drain buffered messages then see
    /// `RecvError::Closed`.
    fn close(&self);
}

/// Port: Execute named CLI tools (plugins).
///
/// This port abstracts plugin execution, allowing the domain layer to request
/// tool execution without knowing about subprocess details, path resolution, or
/// tool availability probing.
///
/// Implementors must:
/// - Handle tool resolution (PATH lookup)
/// - Execute the tool with arguments
/// - Return process output or errors
pub trait PluginExecutor: Send + Sync {
    /// Check if a named tool is available in PATH.
    ///
    /// This is a fast, non-execution check for tool presence.
    fn has_tool(&self, tool: &str) -> bool;

    /// Execute a named tool, requiring it to exist.
    ///
    /// Returns an error if the tool is not found in PATH.
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;

    /// Execute a named tool if it exists; otherwise return None.
    ///
    /// This is the "soft" version of `execute_tool` for optional tools.
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;

    /// Probe if a named tool exists and succeeds with given arguments.
    ///
    /// Returns true if the tool is available and exits with status 0.
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
