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
    /// Plugin is running and healthy.
    Healthy,
    /// Plugin is running but unhealthy.
    Unhealthy,
    /// Plugin is intentionally disabled.
    Disabled,
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
}

/// Port for orchestration plugins that choose agents from prompts.
pub trait OrchestrationPluginPort: Send + Sync {
    /// Select an agent for `prompt`, or `None` when no override applies.
    fn select_agent_for_prompt(
        &mut self,
        prompt: &str,
    ) -> anyhow::Result<Option<PluginAgentSelection>>;
}

/// Port for supervising tool plugins.
pub trait ToolSupervisorPort: Send + Sync {
    /// Return current supervisor status for `plugin_name`.
    fn status(&self, plugin_name: &str) -> Option<PluginSupervisorStatus>;
    /// Request a plugin restart with a human-readable `reason`.
    fn restart(&mut self, plugin_name: &str, reason: &str) -> anyhow::Result<()>;
}

/// Port for supervising runtime plugins.
pub trait RuntimeSupervisorPort: Send + Sync {
    /// Return current supervisor status for `plugin_name`.
    fn status(&self, plugin_name: &str) -> Option<PluginSupervisorStatus>;
    /// Request a plugin restart with a human-readable `reason`.
    fn restart(&mut self, plugin_name: &str, reason: &str) -> anyhow::Result<()>;
}

/// Port for supervising orchestration plugins.
pub trait OrchestrationSupervisorPort: Send + Sync {
    /// Return current supervisor status for `plugin_name`.
    fn status(&self, plugin_name: &str) -> Option<PluginSupervisorStatus>;
    /// Request a plugin restart with a human-readable `reason`.
    fn restart(&mut self, plugin_name: &str, reason: &str) -> anyhow::Result<()>;
}
