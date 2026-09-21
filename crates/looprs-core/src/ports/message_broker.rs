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
    /// Creates a timestamped message for the given source and topic.
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

// Broker port

/// Port: fan-out message broker for inter-component pub/sub.
///
/// Implementations must be cheaply cloneable (`Arc`-backed) so callers
/// can hold a handle without worrying about lifetimes.
pub trait MessageBroker: Send + Sync {
    /// Publishes a message and returns the number of active subscribers.
    fn publish(&self, msg: Message) -> usize;
    /// Subscribes to messages published on the given topic.
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;
    /// Closes the broker and disconnects all topic subscribers.
    fn close(&self);
}
