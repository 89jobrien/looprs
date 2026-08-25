//! Domain types for structured LLM output.
//!
//! UI-only types have been discarded. The remaining types use serde directly.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// How urgent a detected [`DataAnomaly`] is, from `Critical` down to `Info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// The kind of deviation that produced a [`DataAnomaly`].
///
/// Describes *how* the anomaly was identified rather than how serious it is;
/// see [`AnomalySeverity`] for urgency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnomalyType {
    Statistical,
    Semantic,
    Temporal,
    Structural,
    Contextual,
}

/// Coarse health state for a system or one of its components.
///
/// `Unknown` means health could not be determined, which is distinct from
/// `Degraded` (determined, and bad).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

/// Top-level classification of what a user is trying to accomplish.
///
/// Used as the coarse bucket in [`UserIntent`]; finer detail goes in
/// [`UserIntent::sub_intent`].
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

/// Overall emotional tone attributed to a message.
///
/// Coarser than [`Sentiment`]; `Mixed` indicates competing signals rather than
/// an absence of signal (which is `Neutral`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mood {
    Positive,
    Negative,
    Neutral,
    Mixed,
}

/// Graded sentiment polarity on a five-point scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Sentiment {
    VeryPositive,
    Positive,
    Neutral,
    Negative,
    VeryNegative,
}

/// General-purpose severity scale for findings that are not anomalies.
///
/// Mirrors [`AnomalySeverity`] but applies to generic diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Position of a workflow in its lifecycle.
///
/// `Complete` and `Failed` are terminal; the rest are transitional.
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

/// Structured interpretation of a single user message.
///
/// Aggregates intent, sentiment, and extracted topics/entities into one
/// payload so downstream consumers need only one LLM round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAnalysis {
    /// What the user is trying to do.
    pub intent: UserIntent,
    /// Emotional tone of the message.
    pub sentiment: SentimentContext,
    /// Subject areas the message touches on.
    pub topics: Vec<String>,
    /// Named entities (files, commands, identifiers) mentioned.
    pub entities: Vec<String>,
    /// Whether the message expects the agent to act rather than just reply.
    pub requires_action: bool,
    /// Model confidence in this analysis, in the range `0.0..=1.0`.
    pub confidence: f32,
}

/// A classified user goal, with optional refinement and extracted arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntent {
    /// Coarse intent bucket.
    pub category: IntentCategory,
    /// Optional finer-grained intent within [`Self::category`].
    pub sub_intent: Option<String>,
    /// Arguments or operands extracted from the message.
    pub parameters: Vec<String>,
    /// Model confidence in this classification, in the range `0.0..=1.0`.
    pub confidence: f32,
}

/// Aggregate health report for the system and its components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Rolled-up status across all components.
    pub status: HealthStatus,
    /// Per-component breakdown behind [`Self::status`].
    pub components: Vec<ComponentHealth>,
    /// Composite score in the range `0.0..=1.0`, higher is healthier.
    pub overall_score: f32,
    /// Suggested remediation steps, ordered by importance.
    pub recommendations: Vec<String>,
}

/// Health of a single named component within a [`SystemHealth`] report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component identifier.
    pub name: String,
    /// Status for this component alone.
    pub status: HealthStatus,
    /// Optional human-readable detail, typically set when not `Healthy`.
    pub message: Option<String>,
}

/// Snapshot of a workflow's progress at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Current lifecycle stage.
    pub stage: WorkflowStage,
    /// Completion fraction in the range `0.0..=1.0`.
    pub progress: f32,
    /// Task currently executing, if any.
    pub current_task: Option<String>,
    /// Tasks already finished, in completion order.
    pub completed_tasks: Vec<String>,
    /// Tasks not yet started, in planned order.
    pub pending_tasks: Vec<String>,
    /// Errors encountered so far; non-empty does not necessarily mean
    /// [`WorkflowStage::Failed`], since some errors are recoverable.
    pub errors: Vec<String>,
}

/// Sentiment analysis result with polarity, mood, and targeted aspects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentContext {
    /// Graded polarity.
    pub sentiment: Sentiment,
    /// Coarse emotional tone.
    pub mood: Mood,
    /// Strength of the sentiment signal, in the range `0.0..=1.0`.
    pub intensity: f32,
    /// Specific aspects the sentiment is directed at.
    pub aspects: Vec<String>,
}

/// A single detected deviation in observed data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAnomaly {
    /// How the anomaly was identified.
    pub anomaly_type: AnomalyType,
    /// How urgent the anomaly is.
    pub severity: AnomalySeverity,
    /// Human-readable explanation of what was detected.
    pub description: String,
    /// Fields or dimensions the anomaly applies to.
    pub affected_fields: Vec<String>,
    /// Model confidence in this detection, in the range `0.0..=1.0`.
    pub confidence: f32,
}
