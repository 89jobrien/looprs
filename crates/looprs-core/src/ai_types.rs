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
    /// Immediate attention required.
    Critical,
    /// Serious issue that should be addressed soon.
    High,
    /// Moderate impact issue.
    Medium,
    /// Minor issue with limited impact.
    Low,
    /// Informational signal only.
    Info,
}

/// The kind of deviation that produced a [`DataAnomaly`].
///
/// Describes *how* the anomaly was identified rather than how serious it is;
/// see [`AnomalySeverity`] for urgency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Outlier detected from numeric distribution behavior.
    Statistical,
    /// Meaning/content inconsistency.
    Semantic,
    /// Time-series pattern deviation.
    Temporal,
    /// Unexpected shape or schema arrangement.
    Structural,
    /// Context-dependent mismatch.
    Contextual,
}

/// Coarse health state for a system or one of its components.
///
/// `Unknown` means health could not be determined, which is distinct from
/// `Degraded` (determined, and bad).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Operating normally.
    Healthy,
    /// Operating but below expected quality.
    Degraded,
    /// Functionally impaired or near outage.
    Critical,
    /// Insufficient data to classify health.
    Unknown,
}

/// Top-level classification of what a user is trying to accomplish.
///
/// Used as the coarse bucket in [`UserIntent`]; finer detail goes in
/// [`UserIntent::sub_intent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntentCategory {
    /// User is asking for information.
    Query,
    /// User is asking to execute an action.
    Command,
    /// User wants interpretation or diagnosis.
    Analysis,
    /// User is asking to create something new.
    Creation,
    /// User is asking to change existing content.
    Modification,
    /// User is asking to remove something.
    Deletion,
    /// User is asking to move/search within a space.
    Navigation,
    /// User is asking for guidance.
    Help,
    /// Could not confidently classify intent.
    Unknown,
}

/// Overall emotional tone attributed to a message.
///
/// Coarser than [`Sentiment`]; `Mixed` indicates competing signals rather than
/// an absence of signal (which is `Neutral`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mood {
    /// Overall positive tone.
    Positive,
    /// Overall negative tone.
    Negative,
    /// Flat or emotionally neutral tone.
    Neutral,
    /// Mixed positive and negative cues.
    Mixed,
}

/// Graded sentiment polarity on a five-point scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Sentiment {
    /// Strongly positive polarity.
    VeryPositive,
    /// Positive polarity.
    Positive,
    /// Neutral polarity.
    Neutral,
    /// Negative polarity.
    Negative,
    /// Strongly negative polarity.
    VeryNegative,
}

/// General-purpose severity scale for findings that are not anomalies.
///
/// Mirrors [`AnomalySeverity`] but applies to generic diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    /// Immediate attention required.
    Critical,
    /// Serious issue that should be addressed soon.
    High,
    /// Moderate impact issue.
    Medium,
    /// Minor issue with limited impact.
    Low,
    /// Informational finding only.
    Info,
}

/// Position of a workflow in its lifecycle.
///
/// `Complete` and `Failed` are terminal; the rest are transitional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStage {
    /// No active workflow execution.
    Idle,
    /// Determining next actions.
    Planning,
    /// Carrying out planned work.
    Executing,
    /// Evaluating output and quality.
    Reviewing,
    /// Workflow finished successfully.
    Complete,
    /// Workflow ended due to failure.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_analysis_roundtrips() {
        let analysis = MessageAnalysis {
            intent: UserIntent {
                category: IntentCategory::Modification,
                sub_intent: Some("rename function".into()),
                parameters: vec!["agent.rs".into()],
                confidence: 0.9,
            },
            sentiment: SentimentContext {
                sentiment: Sentiment::Neutral,
                mood: Mood::Mixed,
                intensity: 0.4,
                aspects: vec!["naming".into()],
            },
            topics: vec!["refactoring".into()],
            entities: vec!["agent.rs".into()],
            requires_action: true,
            confidence: 0.85,
        };

        let json = serde_json::to_string(&analysis).unwrap();
        let back: MessageAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(back.intent.category, IntentCategory::Modification);
        assert_eq!(back.sentiment.mood, Mood::Mixed);
        assert!(back.requires_action);
        assert_eq!(back.entities, vec!["agent.rs".to_string()]);
    }

    #[test]
    fn system_health_roundtrips_with_components() {
        let health = SystemHealth {
            status: HealthStatus::Degraded,
            components: vec![ComponentHealth {
                name: "db".into(),
                status: HealthStatus::Critical,
                message: Some("connection pool exhausted".into()),
            }],
            overall_score: 0.42,
            recommendations: vec!["restart db".into()],
        };

        let json = serde_json::to_string(&health).unwrap();
        let back: SystemHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status, HealthStatus::Degraded);
        assert_eq!(back.components[0].status, HealthStatus::Critical);
        assert_eq!(
            back.components[0].message.as_deref(),
            Some("connection pool exhausted")
        );
    }

    #[test]
    fn workflow_state_roundtrips_terminal_stage() {
        let state = WorkflowState {
            stage: WorkflowStage::Failed,
            progress: 0.8,
            current_task: None,
            completed_tasks: vec!["step-1".into(), "step-2".into()],
            pending_tasks: vec![],
            errors: vec!["timeout on step-3".into()],
        };

        let back: WorkflowState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(back.stage, WorkflowStage::Failed);
        assert_eq!(back.completed_tasks.len(), 2);
        assert_eq!(back.errors.len(), 1);
    }

    #[test]
    fn data_anomaly_serializes_variant_names() {
        let anomaly = DataAnomaly {
            anomaly_type: AnomalyType::Statistical,
            severity: AnomalySeverity::High,
            description: "spike in latency".into(),
            affected_fields: vec!["p99".into()],
            confidence: 0.95,
        };

        let json = serde_json::to_value(&anomaly).unwrap();
        assert_eq!(json["anomaly_type"], "Statistical");
        assert_eq!(json["severity"], "High");
    }

    #[test]
    fn sentiment_enum_serializes_graded_variants() {
        for (value, expected) in [
            (Sentiment::VeryPositive, "VeryPositive"),
            (Sentiment::VeryNegative, "VeryNegative"),
        ] {
            let json = serde_json::to_value(value).unwrap();
            assert_eq!(json, expected);
        }
    }
}
