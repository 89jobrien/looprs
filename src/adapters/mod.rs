//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Adapters bridge the gap between domain ports and concrete infrastructure.

pub mod plugin_executor;

pub use plugin_executor::PluginsAdapter;
