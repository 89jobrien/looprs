use serde::{Deserialize, Serialize};

/// Identifies a pipeline run, e.g. for correlating log entries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineContext {
    /// Unique identifier for the run, if assigned.
    pub run_id: Option<String>,
}

/// The outcome of a single pipeline check (e.g. `"build"`, `"lint"`,
/// `"tests"`, `"typecheck"`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepResult {
    /// The check's name.
    pub step: String,
    /// Whether the check's underlying command exited successfully.
    pub success: bool,
}

/// A captured tool invocation and its output, recorded alongside a
/// pipeline run for later analysis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    /// Name of the tool that was invoked.
    pub tool: String,
    /// The tool's output, as JSON.
    pub output: serde_json::Value,
}

/// A scalar quality score computed for a pipeline run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardReport {
    /// The computed reward value; range and higher-is-better/lower-is-better
    /// convention are defined by whichever scorer produced it.
    pub reward: f64,
}

/// The full result of running
/// [`crate::pipeline::PipelineRunner::run_checks`]: which checks ran and
/// whether they passed, any tool invocations recorded, and an optional
/// computed reward.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineReport {
    /// Result of each enabled check, in execution order.
    pub steps: Vec<StepResult>,
    /// Tool invocations recorded for this run. Currently always empty;
    /// [`crate::pipeline::PipelineRunner::run_checks`] does not populate
    /// it.
    pub tools: Vec<ToolResult>,
    /// Computed reward for this run, if scoring was performed. Currently
    /// always `None`; [`crate::pipeline::PipelineRunner::run_checks`] does
    /// not populate it.
    pub reward: Option<RewardReport>,
}
