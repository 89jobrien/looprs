//! Ports (hexagonal architecture) — outbound interfaces to external systems.
//!
//! Ports define what the application domain needs from external infrastructure,
//! not how those needs are fulfilled. Adapters provide concrete implementations.

use std::ffi::OsString;
use std::process::Output;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ── Domain type ─────────────────────────────────────────────────────────

/// A message routed through the pub/sub broker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub topic: String,
    pub schema_version: u32,
    pub payload: serde_json::Value,
}

impl Message {
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

// ── Ports ───────────────────────────────────────────────────────────────

/// Port: fan-out message broker for inter-component pub/sub.
///
/// Implementations must be cheaply cloneable (`Arc`-backed) so callers
/// can hold a handle without worrying about lifetimes.
pub trait MessageBroker: Send + Sync {
    fn publish(&self, msg: Message) -> usize;
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;
    fn close(&self);
}

/// Port: Execute named CLI tools (plugins).
///
/// Abstracts plugin execution so the domain layer can request tool
/// execution without knowing about subprocess details or PATH resolution.
pub trait PluginExecutor: Send + Sync {
    fn has_tool(&self, tool: &str) -> bool;
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
