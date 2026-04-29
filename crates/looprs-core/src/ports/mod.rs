//! Ports (hexagonal architecture) — outbound interfaces to external systems.
//!
//! Ports define what the application domain needs from external infrastructure,
//! not how those needs are fulfilled. Adapters provide concrete implementations.

pub mod message_broker;
pub mod plugin_executor;
pub mod session_store;
pub mod user_output;

// Re-export all port traits and the Message domain type.
pub use message_broker::{Message, MessageBroker};
pub use plugin_executor::PluginExecutor;
pub use session_store::{SessionEvent, SessionStore};
pub use user_output::UserOutput;
