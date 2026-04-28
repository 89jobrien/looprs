//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Adapters bridge the gap between domain ports and concrete infrastructure.

pub mod channel_broker;
pub mod plugin_executor;
pub mod retry_provider;

pub use channel_broker::ChannelBroker;
pub use plugin_executor::PluginsAdapter;
pub use retry_provider::RetryProvider;
