//! Ports (hexagonal architecture) — outbound interfaces to external systems.
//!
//! Ports define what the application domain needs from external infrastructure,
//! not how those needs are fulfilled. Adapters provide concrete implementations.

pub mod inference_provider;
pub mod message_broker;
pub mod observation_store;
pub mod plugin_executor;
pub mod session_store;
pub mod user_output;

#[cfg(any(test, feature = "test-contracts"))]
pub mod test_contracts;

// Re-export all port traits and the Message domain type.
pub use inference_provider::{
    InferStream, InferenceProvider, InferenceRequest, InferenceResponse, Usage,
};
pub use message_broker::{Message, MessageBroker};
pub use observation_store::ObservationStore;
pub use plugin_executor::PluginExecutor;
pub use session_store::{SessionEvent, SessionStore};
pub use user_output::UserOutput;
