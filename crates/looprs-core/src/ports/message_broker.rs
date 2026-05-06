//! MessageBroker port — fan-out pub/sub message routing.

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

// ── Port ─────────────────────────────────────────────────────────────────

/// Port: fan-out message broker for inter-component pub/sub.
///
/// Implementations must be cheaply cloneable (`Arc`-backed) so callers
/// can hold a handle without worrying about lifetimes.
pub trait MessageBroker: Send + Sync {
    fn publish(&self, msg: Message) -> usize;
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;
    fn close(&self);
}
