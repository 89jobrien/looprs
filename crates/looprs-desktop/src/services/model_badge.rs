use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelBadgeState {
    pub model_id: String,
    pub mean_reward: f32,
    pub training_status: String,
}

#[derive(Deserialize, Default)]
struct Modelcard {
    #[serde(default)]
    model_id: String,
    #[serde(default)]
    training_status: String,
    #[serde(default)]
    eval_results: BTreeMap<String, serde_yaml::Value>,
}

pub fn load_badge_state(modelcard_path: &Path) -> ModelBadgeState {
    let content = match std::fs::read_to_string(modelcard_path) {
        Ok(c) => c,
        Err(_) => {
            return ModelBadgeState {
                model_id: "unknown".into(),
                mean_reward: 0.0,
                training_status: "unknown".into(),
            }
        }
    };
    let mc: Modelcard = match serde_yaml::from_str(&content) {
        Ok(m) => m,
        Err(_) => {
            return ModelBadgeState {
                model_id: "unknown".into(),
                mean_reward: 0.0,
                training_status: "unknown".into(),
            }
        }
    };
    let mut all_rewards: Vec<f32> = mc
        .eval_results
        .values()
        .filter_map(|v| v.get("mean_reward")?.as_f64())
        .map(|f| f as f32)
        .collect();
    let rewards: Vec<f32> = {
        let len = all_rewards.len();
        let start = len.saturating_sub(50);
        all_rewards.drain(start..).collect()
    };
    let mean = if rewards.is_empty() {
        0.0
    } else {
        rewards.iter().sum::<f32>() / rewards.len() as f32
    };
    ModelBadgeState {
        model_id: if mc.model_id.is_empty() {
            "unknown".into()
        } else {
            mc.model_id
        },
        mean_reward: mean,
        training_status: if mc.training_status.is_empty() {
            "idle".into()
        } else {
            mc.training_status
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_badge_from_fixture() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "model_id: magistral-small-rl-v17").unwrap();
        writeln!(f, "training_status: idle").unwrap();
        writeln!(f, "eval_results:").unwrap();
        writeln!(f, "  code_review:").unwrap();
        writeln!(f, "    mean_reward: 0.82").unwrap();
        writeln!(f, "  debugging:").unwrap();
        writeln!(f, "    mean_reward: 0.74").unwrap();
        let state = load_badge_state(f.path());
        assert_eq!(state.model_id, "magistral-small-rl-v17");
        assert!(state.mean_reward >= 0.0 && state.mean_reward <= 1.0);
        assert_eq!(state.training_status, "idle");
    }

    #[test]
    fn test_missing_modelcard_returns_unknown() {
        let state = load_badge_state(std::path::Path::new("/nonexistent/modelcard.yaml"));
        assert_eq!(state.model_id, "unknown");
        assert_eq!(state.training_status, "unknown");
    }
}
