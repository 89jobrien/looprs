//! MessageBroker port — fan-out pub/sub message routing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ── Domain type ─────────────────────────────────────────────────────────

/// A message routed through the pub/sub broker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Identifier of the component that published this message.
    pub source: String,
    /// Publication time, set automatically by [`Message::new`].
    pub timestamp: DateTime<Utc>,
    /// Routing key subscribers match against.
    pub topic: String,
    /// Version of the [`Message::payload`] shape, so consumers can handle
    /// older producers rather than failing to deserialize.
    pub schema_version: u32,
    /// Message body; its shape is determined by `topic` and
    /// `schema_version`.
    pub payload: serde_json::Value,
}

impl Message {
    /// Build a message stamped with the current UTC time.
    ///
    /// The caller supplies `schema_version` explicitly; it is not inferred
    /// from the payload.
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
    /// Broadcast `msg` to every subscriber of its topic.
    ///
    /// Returns the number of subscribers the message was delivered to; `0`
    /// means nobody was listening, which is not an error.
    fn publish(&self, msg: Message) -> usize;

    /// Subscribe to `topic`, returning a receiver for future messages.
    ///
    /// Only messages published after subscribing are delivered. A slow
    /// consumer that lets its buffer overflow will observe
    /// [`broadcast::error::RecvError::Lagged`] and skip messages.
    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message>;

    /// Shut the broker down and drop all subscriptions.
    ///
    /// Outstanding receivers observe channel closure. Publishing after this
    /// point delivers to nobody.
    fn close(&self);
}
