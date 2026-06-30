//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Portable adapters live in `looprs_core::adapters` and are re-exported here
//! for backwards compatibility. Adapters that depend on looprs internals
//! (`PluginsAdapter`, `RetryProvider`) remain in this module.

pub mod plugin_executor;
pub mod retry_provider;
pub mod sqlite_session_store;
pub mod ui_output;

// Re-export portable adapters from looprs-core.
pub use looprs_core::adapters::ChannelBroker;
pub use looprs_core::adapters::FsSessionStore;
pub use looprs_core::adapters::NullOutput;
pub use looprs_core::adapters::TerminalOutput;
pub use plugin_executor::PluginsAdapter;
pub use retry_provider::RetryProvider;
pub use sqlite_session_store::SqliteSessionStore;
pub use ui_output::UiOutput;

use crate::ports::SessionStore;

/// Create the default session store with fallback logic.
///
/// Tries `~/.looprs/sessions/` first, then `$TMPDIR/looprs-sessions/`.
/// Returns `None` if neither location is writable.
pub fn default_session_store() -> Option<Box<dyn SessionStore>> {
    let primary = dirs::home_dir().map(|h| h.join(".looprs").join("sessions"));
    primary
        .and_then(|d| FsSessionStore::new(d).ok())
        .map(|s| Box::new(s) as Box<dyn SessionStore>)
        .or_else(
            || match FsSessionStore::new(std::env::temp_dir().join("looprs-sessions")) {
                Ok(logger) => Some(Box::new(logger) as Box<dyn SessionStore>),
                Err(e) => {
                    log::warn!("failed to initialize fallback session logger: {e}");
                    None
                }
            },
        )
}
