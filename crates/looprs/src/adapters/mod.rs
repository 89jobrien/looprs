//! Adapters (hexagonal architecture) — implementations of ports.
//!
//! Portable adapters live in `looprs_core::adapters` and are re-exported here
//! for backwards compatibility. Adapters that depend on looprs internals
//! (`PluginsAdapter`, `RetryProvider`) remain in this module.

pub mod mcp_executor;
pub mod plugin_executor;
pub mod retry_provider;
pub mod sqlite_observation_store;
pub mod sqlite_session_store;
pub mod ui_output;

// Re-export portable adapters from looprs-core.
pub use looprs_core::adapters::ChannelBroker;
pub use looprs_core::adapters::FsSessionStore;
pub use looprs_core::adapters::NullOutput;
pub use looprs_core::adapters::TerminalOutput;
pub use mcp_executor::{McpToolCatalog, McpToolExecutor};
pub use plugin_executor::PluginsAdapter;
pub use retry_provider::RetryProvider;
pub use sqlite_observation_store::SqliteObservationStore;
pub use sqlite_session_store::SqliteSessionStore;
pub use ui_output::UiOutput;

use crate::app_config::{AppConfig, SessionStoreBackend};
use crate::errors::AgentError;
use crate::file_refs::FileRefPolicy;
use crate::ports::SessionStore;
use crate::ports::UserOutput;
use crate::providers::LLMProvider;
use crate::tools::{BuiltinToolCatalog, DefaultToolExecutor, ToolPorts};
use crate::{Agent, RuntimeSettings};
use std::sync::Arc;

/// Compose an agent with the default runtime adapters.
pub fn default_agent(provider: Box<dyn LLMProvider>) -> Result<Agent, AgentError> {
    agent_with_runtime(
        provider,
        RuntimeSettings::default(),
        FileRefPolicy::default(),
        None,
        Box::new(UiOutput),
    )
}

/// Compose an agent from runtime settings and default tool adapters.
pub fn agent_with_runtime(
    provider: Box<dyn LLMProvider>,
    runtime: RuntimeSettings,
    file_ref_policy: FileRefPolicy,
    session_logger: Option<Box<dyn SessionStore>>,
    output: Box<dyn UserOutput>,
) -> Result<Agent, AgentError> {
    let tool_ports = default_tool_ports(runtime.mcp_server_url());
    Agent::new_with_runtime_and_tool_ports(
        provider,
        runtime,
        file_ref_policy,
        session_logger,
        output,
        tool_ports,
    )
}

/// Apply runtime settings and rebuild default tool adapters at the composition root.
pub fn apply_runtime_settings(agent: &mut Agent, runtime: RuntimeSettings) {
    agent.set_tool_ports(default_tool_ports(runtime.mcp_server_url()));
    agent.set_runtime_settings(runtime);
}

/// Compose the default tool catalog and dispatcher for runtime settings.
pub fn default_tool_ports(mcp_server_url: Option<&str>) -> ToolPorts {
    let Some(server_url) = mcp_server_url else {
        return ToolPorts::builtin();
    };

    ToolPorts::new(
        Arc::new(McpToolCatalog::new(
            server_url,
            Arc::new(BuiltinToolCatalog),
        )),
        Arc::new(McpToolExecutor::with_fallback(
            server_url,
            Box::new(DefaultToolExecutor),
        )),
    )
}

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
