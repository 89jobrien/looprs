//! Domain types for structured LLM output.
//!
//! UI-only types have been discarded. The remaining types use serde directly.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnomalyType {
    Statistical,
    Semantic,
    Temporal,
    Structural,
    Contextual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntentCategory {
    Query,
    Command,
    Analysis,
    Creation,
    Modification,
    Deletion,
    Navigation,
    Help,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mood {
    Positive,
    Negative,
    Neutral,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Sentiment {
    VeryPositive,
    Positive,
    Neutral,
    Negative,
    VeryNegative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStage {
    Idle,
    Planning,
    Executing,
    Reviewing,
    Complete,
    Failed,
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAnalysis {
    pub intent: UserIntent,
    pub sentiment: SentimentContext,
    pub topics: Vec<String>,
    pub entities: Vec<String>,
    pub requires_action: bool,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntent {
    pub category: IntentCategory,
    pub sub_intent: Option<String>,
    pub parameters: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub components: Vec<ComponentHealth>,
    pub overall_score: f32,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub stage: WorkflowStage,
    pub progress: f32,
    pub current_task: Option<String>,
    pub completed_tasks: Vec<String>,
    pub pending_tasks: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentContext {
    pub sentiment: Sentiment,
    pub mood: Mood,
    pub intensity: f32,
    pub aspects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAnomaly {
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub description: String,
    pub affected_fields: Vec<String>,
    pub confidence: f32,
}
