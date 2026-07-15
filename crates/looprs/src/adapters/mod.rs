//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Portable adapters live in `looprs_core::adapters` and are re-exported here
//! for backwards compatibility. Adapters that depend on looprs internals
//! (`PluginsAdapter`, `RetryProvider`) remain in this module.

pub mod mcp_executor;
pub mod plugin_executor;
pub mod retry_provider;
pub mod sqlite_session_store;
pub mod ui_output;

// Re-export portable adapters from looprs-core.
pub use looprs_core::adapters::ChannelBroker;
pub use looprs_core::adapters::FsSessionStore;
pub use looprs_core::adapters::NullOutput;
pub use looprs_core::adapters::TerminalOutput;
pub use mcp_executor::McpToolExecutor;
pub use plugin_executor::PluginsAdapter;
pub use retry_provider::RetryProvider;
pub use sqlite_session_store::SqliteSessionStore;
pub use ui_output::UiOutput;

use crate::app_config::{AppConfig, SessionStoreBackend};
use crate::ports::SessionStore;

/// Create the session store selected by `persistence.session_store` in config.
///
/// - `sqlite` → `SqliteSessionStore` at `~/.looprs/sessions.db`
/// - `fs` (default) → `FsSessionStore` at `~/.looprs/sessions/`, with a
///   `$TMPDIR/looprs-sessions/` fallback if that path is not writable.
///
/// Returns `None` only when the selected backend cannot be initialised.
pub fn default_session_store() -> Option<Box<dyn SessionStore>> {
    let backend = AppConfig::load()
        .ok()
        .map(|c| c.persistence.session_store)
        .unwrap_or_default();

    match backend {
        SessionStoreBackend::Sqlite => {
            let db_path = dirs::home_dir()?.join(".looprs").join("sessions.db");
            match SqliteSessionStore::new(db_path) {
                Ok(store) => Some(Box::new(store) as Box<dyn SessionStore>),
                Err(e) => {
                    log::warn!("failed to open SQLite session store: {e}");
                    None
                }
            }
        }
        SessionStoreBackend::Fs => {
            let primary = dirs::home_dir().map(|h| h.join(".looprs").join("sessions"));
            primary
                .and_then(|d| FsSessionStore::new(d).ok())
                .map(|s| Box::new(s) as Box<dyn SessionStore>)
                .or_else(|| {
                    match FsSessionStore::new(std::env::temp_dir().join("looprs-sessions")) {
                        Ok(logger) => Some(Box::new(logger) as Box<dyn SessionStore>),
                        Err(e) => {
                            log::warn!("failed to initialize fallback session logger: {e}");
                            None
                        }
                    }
                })
        }
    }
}
