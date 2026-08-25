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
}
