//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Portable adapters live in `looprs_core::adapters` and are re-exported here
//! for backwards compatibility. Adapters that depend on looprs internals
//! (`PluginsAdapter`, `RetryProvider`) remain in this module.

pub mod plugin_executor;
pub mod retry_provider;

// Re-export portable adapters from looprs-core.
pub use looprs_core::adapters::ChannelBroker;
pub use looprs_core::adapters::FsSessionStore;
pub use looprs_core::adapters::TerminalOutput;
pub use plugin_executor::PluginsAdapter;
pub use retry_provider::RetryProvider;
