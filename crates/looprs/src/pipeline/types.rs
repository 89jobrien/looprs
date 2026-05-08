use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineContext {
    pub run_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepResult {
    pub step: String,
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: String,
    pub output: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardReport {
    pub reward: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineReport {
    pub steps: Vec<StepResult>,
    pub tools: Vec<ToolResult>,
    pub reward: Option<RewardReport>,
}
