//! ChannelBroker adapter — `MessageBroker` port backed by `tokio::sync::broadcast`.
//!
//! Each topic gets its own broadcast channel with capacity 64. Publishers fan-out
//! to all subscribers; slow subscribers lag (messages are dropped) rather than
//! blocking the publisher.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::ports::{Message, MessageBroker};

const CHANNEL_CAPACITY: usize = 64;

/// Thread-safe, cheaply cloneable pub/sub broker backed by tokio broadcast channels.
///
/// # Example
///
/// ```rust
/// use looprs_core::adapters::channel_broker::ChannelBroker;
/// use looprs_core::ports::{Message, MessageBroker};
///
/// let broker = ChannelBroker::new();
/// let mut rx = broker.subscribe("my.topic");
/// broker.publish(Message::new("test", "my.topic", 1, serde_json::Value::Null));
/// // rx.try_recv() would return the message
/// ```
#[derive(Clone)]
pub struct ChannelBroker {
    inner: Arc<BrokerInner>,
}

struct BrokerInner {
    /// Topic → broadcast sender.  A new sender is created on first subscribe/publish.
    topics: Mutex<HashMap<String, broadcast::Sender<Message>>>,
    closed: Mutex<bool>,
}

impl ChannelBroker {
    /// Create a new, open broker.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BrokerInner {
                topics: Mutex::new(HashMap::new()),
                closed: Mutex::new(false),
            }),
        }
    }

    /// Return (or create) the sender for `topic`.
    fn sender_for(&self, topic: &str) -> Option<broadcast::Sender<Message>> {
        let closed = self.inner.closed.lock().expect("closed lock poisoned");
        if *closed {
            return None;
        }
        drop(closed);

        let mut topics = self.inner.topics.lock().expect("topics lock poisoned");
        Some(
            topics
                .entry(topic.to_owned())
                .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
                .clone(),
        )
    }
}

impl Default for ChannelBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBroker for ChannelBroker {
    fn publish(&self, msg: Message) -> usize {
        let topic = msg.topic.clone();
        match self.sender_for(&topic) {
            None => 0,
            Some(tx) => tx.send(msg).unwrap_or_default(),
        }
    }

    fn subscribe(&self, topic: &str) -> broadcast::Receiver<Message> {
        match self.sender_for(topic) {
            Some(tx) => tx.subscribe(),
            None => {
                // Broker is closed — return a receiver that immediately sees Closed.
                let (tx, rx) = broadcast::channel(1);
                drop(tx);
                rx
            }
        }
    }

    fn close(&self) {
        let mut closed = self.inner.closed.lock().expect("closed lock poisoned");
        *closed = true;
        // Dropping all senders causes receivers to drain then see RecvError::Closed.
        let mut topics = self.inner.topics.lock().expect("topics lock poisoned");
        topics.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast::error::TryRecvError;

    #[test]
    fn publish_to_subscriber() {
        let broker = ChannelBroker::new();
        let mut rx = broker.subscribe("a");

        let msg = Message::new("src", "a", 1, serde_json::json!({"x": 1}));
        let n = broker.publish(msg.clone());

        assert_eq!(n, 1);
        let received = rx.try_recv().expect("should have message");
        assert_eq!(received.topic, "a");
        assert_eq!(received.source, "src");
    }

    #[test]
    fn no_subscribers_returns_zero() {
        let broker = ChannelBroker::new();
        let n = broker.publish(Message::new(
            "src",
            "unsubscribed",
            1,
            serde_json::Value::Null,
        ));
        assert_eq!(n, 0);
    }

    #[test]
    fn multiple_subscribers_same_topic() {
        let broker = ChannelBroker::new();
        let mut rx1 = broker.subscribe("t");
        let mut rx2 = broker.subscribe("t");

        let n = broker.publish(Message::new("src", "t", 1, serde_json::Value::Null));
        assert_eq!(n, 2);
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn independent_topics_do_not_cross() {
        let broker = ChannelBroker::new();
        let mut rx_a = broker.subscribe("a");
        let mut _rx_b = broker.subscribe("b");

        broker.publish(Message::new("src", "a", 1, serde_json::Value::Null));

        assert!(rx_a.try_recv().is_ok());
        // rx_b should see nothing
        assert!(matches!(_rx_b.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn close_makes_receiver_see_closed() {
        let broker = ChannelBroker::new();
        let mut rx = broker.subscribe("c");
        broker.close();

        // After close, publishing is a no-op.
        let n = broker.publish(Message::new("src", "c", 1, serde_json::Value::Null));
        assert_eq!(n, 0);

        // Receiver sees Closed.
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Closed)));
    }

    #[test]
    fn clone_shares_state() {
        let broker = ChannelBroker::new();
        let broker2 = broker.clone();
        let mut rx = broker.subscribe("shared");

        broker2.publish(Message::new("b2", "shared", 1, serde_json::Value::Null));
        assert!(rx.try_recv().is_ok());
    }
}
