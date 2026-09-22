//! Plugin runtime ports — first-class plugin orchestration and supervision.

use serde::{Deserialize, Serialize};

/// High-level plugin category used by runtime supervision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Plugin that exposes tool commands.
    Tool,
    /// Plugin that extends runtime behavior.
    Runtime,
    /// Plugin that selects or orchestrates agent behavior.
    Orchestration,
}

/// Runtime execution mode for a plugin process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginExecutionMode {
    /// Execute once per request and exit.
    #[default]
    OneShot,
    /// Keep a long-lived process running.
    Daemon,
}

/// Selected agent produced by an orchestration plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginAgentSelection {
    /// Plugin responsible for this decision.
    pub plugin_name: String,
    /// Agent name chosen for the prompt.
    pub agent_name: String,
}

/// Health status reported by plugin supervisors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHealthState {
    /// Managed process is running with no observed liveness or probe failure.
    Healthy,
    /// Managed plugin process exited or its configured probe failed.
    Unhealthy,
    /// Plugin is intentionally disabled.
    Disabled,
    /// Managed plugin process was explicitly shut down.
    Stopped,
}

/// Supervisor snapshot for a single plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSupervisorStatus {
    /// Plugin identifier.
    pub plugin_name: String,
    /// Category of this plugin.
    pub kind: PluginKind,
    /// Current supervisor health state.
    pub state: PluginHealthState,
    /// Number of restart attempts performed.
    pub restart_count: u32,
    /// Operating-system process identifier while the daemon is running.
    pub pid: Option<u32>,
    /// Most recent launch, exit, or probe error.
    pub last_error: Option<String>,
    /// Human-readable reason associated with the most recent restart.
    pub last_restart_reason: Option<String>,
}

/// Structured plugin supervision failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginSupervisorError {
    /// The requested manifest does not exist for its kind.
    #[error("unknown {kind:?} plugin '{plugin_name}'")]
    UnknownPlugin {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
    },
    /// The requested manifest is not a managed daemon.
    #[error("cannot supervise {kind:?} plugin '{plugin_name}' because it is not in daemon mode")]
    NotDaemon {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
    },
    /// The requested manifest is intentionally disabled.
    #[error("cannot supervise disabled {kind:?} plugin '{plugin_name}'")]
    Disabled {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
    },
    /// The configured daemon command cannot be resolved or launched.
    #[error("failed to launch {kind:?} plugin '{plugin_name}': {message}")]
    LaunchFailed {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
        /// Actionable process launch error.
        message: String,
    },
    /// The configured health probe failed to execute.
    #[error("failed to probe {kind:?} plugin '{plugin_name}': {message}")]
    ProbeFailed {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
        /// Actionable probe error.
        message: String,
    },
    /// The configured restart budget is exhausted.
    #[error("restart limit ({limit}) reached for {kind:?} plugin '{plugin_name}'")]
    RestartLimitReached {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
        /// Maximum restart attempts allowed for this process.
        limit: u32,
    },
    /// The operating system rejected process shutdown.
    #[error("failed to shut down {kind:?} plugin '{plugin_name}': {message}")]
    ShutdownFailed {
        /// Requested plugin category.
        kind: PluginKind,
        /// Requested plugin identifier.
        plugin_name: String,
        /// Actionable shutdown error.
        message: String,
    },
    /// Manifest refresh failed before the lifecycle operation could run.
    #[error("failed to refresh plugin manifests: {message}")]
    RefreshFailed {
        /// Actionable manifest loading error.
        message: String,
    },
}

/// Port for orchestration plugins that choose agents from prompts.
pub trait OrchestrationPluginPort: Send + Sync {
    /// Select an agent for `prompt`, or `None` when no override applies.
    fn select_agent_for_prompt(
        &mut self,
        prompt: &str,
    ) -> anyhow::Result<Option<PluginAgentSelection>>;
}

/// Port for managed daemon lifecycle operations across every plugin kind.
///
/// One-shot manifests are intentionally outside this contract: they execute at
/// their request site and cannot be restarted or shut down as resident processes.
pub trait PluginSupervisorPort: Send + Sync {
    /// Return the latest process status, refreshing liveness before returning.
    fn status(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<PluginSupervisorStatus, PluginSupervisorError>;

    /// Check process liveness and run the optional manifest health probe.
    fn probe(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<PluginSupervisorStatus, PluginSupervisorError>;

    /// Replace a managed process within its bounded restart budget.
    fn restart(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
        reason: &str,
    ) -> Result<(), PluginSupervisorError>;

    /// Stop a managed process and retain an observable stopped status.
    fn shutdown(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<(), PluginSupervisorError>;
}

/// Capability marker for supervisors that manage tool plugin manifests.
pub trait ToolSupervisorPort: PluginSupervisorPort {}

/// Capability marker for supervisors that manage runtime plugin manifests.
pub trait RuntimeSupervisorPort: PluginSupervisorPort {}

/// Capability marker for supervisors that manage orchestration plugin manifests.
pub trait OrchestrationSupervisorPort: PluginSupervisorPort {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_errors_preserve_kind_name_and_limits() {
        let unknown = PluginSupervisorError::UnknownPlugin {
            kind: PluginKind::Tool,
            plugin_name: "missing".to_string(),
        };
        assert_eq!(unknown.to_string(), "unknown Tool plugin 'missing'");

        let saturated = PluginSupervisorError::RestartLimitReached {
            kind: PluginKind::Runtime,
            plugin_name: "worker".to_string(),
            limit: 3,
        };
        assert!(saturated.to_string().contains("restart limit (3)"));
        assert!(saturated.to_string().contains("Runtime plugin 'worker'"));
    }
}
