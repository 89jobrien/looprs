use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 1. Core domain models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentFacts {
    pub service: String,
    pub symptoms: String,
    pub timeline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub severity: String,
    pub customer_impact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysis {
    pub cause: String,
    pub evidence: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub action: String,
    pub owner: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationPlan {
    pub steps: Vec<PlanStep>,
}

// ---------------------------------------------------------------------------
// 2. Reasoning trace model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StepKind {
    Intent,
    Thought,
    ToolCall,
    Observation,
    StateDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub trace_id: String,
    pub step_index: u32,
    pub kind: StepKind,
    pub timestamp_iso: String,

    pub label: String,
    pub parent_step_index: Option<u32>,

    pub raw_snapshot: Option<String>,
    pub metadata_json: serde_json::Value,
}

// ---------------------------------------------------------------------------
// 3. Ports (hexagonal boundaries) — no I/O details leak in
// ---------------------------------------------------------------------------

#[async_trait]
pub trait LlmPort: Send + Sync {
    async fn complete_text(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<String>;

    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> anyhow::Result<serde_json::Value>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueSpec {
    pub project_key: String,
    pub issue_type: String,
    pub summary: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueRef {
    pub key: String,
    pub url: String,
}

#[async_trait]
pub trait JiraPort: Send + Sync {
    async fn create_issue(&self, spec: JiraIssueSpec) -> anyhow::Result<JiraIssueRef>;
}

#[async_trait]
pub trait TracePort: Send + Sync {
    async fn append_step(&self, step: TraceStep) -> anyhow::Result<()>;
    async fn load_trace(&self, trace_id: &str) -> anyhow::Result<Vec<TraceStep>>;
}
