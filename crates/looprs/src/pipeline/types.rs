//! Defines serializable reports for pipeline steps, tools, rewards, and full runs.

use serde::{Deserialize, Serialize};

/// Outcome of one named pipeline step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepResult {
    /// Stable step name used in reports and logs.
    pub step: String,
    /// Whether the step completed successfully.
    pub success: bool,
}

/// Tool availability or execution metadata captured by the pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool executable name.
    pub tool: String,
    /// Structured metadata about the tool.
    pub output: serde_json::Value,
}

/// Computed ratio of successful checks to attempted checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardReport {
    /// Reward in the inclusive range `0.0..=1.0`.
    pub reward: f64,
}

/// Complete outcome of a pipeline or legacy check run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineReport {
    /// Ordered check and synthetic policy outcomes.
    pub steps: Vec<StepResult>,
    /// Tool metadata, empty for legacy [`crate::pipeline::PipelineRunner::run_checks`].
    pub tools: Vec<ToolResult>,
    /// Computed reward, absent for legacy check-only runs.
    pub reward: Option<RewardReport>,
}

impl PipelineReport {
    /// Return whether every step passes and the reward reaches `threshold`.
    ///
    /// Thresholds below zero become zero, thresholds above one become one, and
    /// `NaN` becomes zero. A reward equal to the normalized threshold passes.
    pub fn succeeds(&self, threshold: f32) -> bool {
        self.steps.iter().all(|step| step.success)
            && self.reward.as_ref().is_none_or(|report| {
                report.reward >= crate::pipeline::normalize_reward_threshold(threshold)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_report() -> PipelineReport {
        PipelineReport {
            steps: vec![
                StepResult {
                    step: "fmt".into(),
                    success: true,
                },
                StepResult {
                    step: "clippy".into(),
                    success: false,
                },
            ],
            tools: vec![ToolResult {
                tool: "nu".into(),
                output: serde_json::json!({"exit": 0}),
            }],
            reward: Some(RewardReport { reward: 0.75 }),
        }
    }

    #[test]
    fn pipeline_report_roundtrips_with_reward() {
        let report = full_report();
        let json = serde_json::to_string(&report).unwrap();
        let back: PipelineReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.steps.len(), 2);
        assert!(!back.steps[1].success);
        assert_eq!(back.reward.as_ref().unwrap().reward, 0.75);
    }

    #[test]
    fn pipeline_report_roundtrips_without_reward() {
        let report = PipelineReport {
            steps: vec![],
            tools: vec![],
            reward: None,
        };
        let json = serde_json::to_string(&report).unwrap();
        let back: PipelineReport = serde_json::from_str(&json).unwrap();
        assert!(back.reward.is_none());
        assert!(back.steps.is_empty());
    }

    #[test]
    fn step_result_serializes_field_names() {
        let json = serde_json::to_value(StepResult {
            step: "test".into(),
            success: true,
        })
        .unwrap();
        assert_eq!(json["step"], "test");
        assert_eq!(json["success"], true);
    }

    #[test]
    fn succeeds_normalizes_threshold_and_accepts_equality() {
        let report = PipelineReport {
            steps: vec![StepResult {
                step: "build".into(),
                success: true,
            }],
            tools: vec![],
            reward: Some(RewardReport { reward: 0.5 }),
        };

        assert!(report.succeeds(f32::NAN));
        assert!(report.succeeds(-1.0));
        assert!(report.succeeds(0.5));
        assert!(!report.succeeds(2.0));
    }
}
