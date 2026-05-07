//! Adapters (hexagonal architecture) — implementations of ports for looprs-core.
//!
//! These adapters are portable: they depend only on `looprs-core` types.
//! Adapters that require `looprs` crate internals (e.g. `PluginsAdapter`,
//! `RetryProvider`) remain in `looprs::adapters`.

pub mod channel_broker;
pub mod fs_session_store;
pub mod null_output;
pub mod terminal_output;

pub use channel_broker::ChannelBroker;
pub use fs_session_store::FsSessionStore;
pub use null_output::NullOutput;
pub use terminal_output::TerminalOutput;
