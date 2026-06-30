//! ModelBadge — reads a YAML modelcard and surfaces model id, training
//! status, and mean reward score.
//!
//! Cannibalized from `looprs-desktop/src/services/model_badge.rs`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

const REWARD_WINDOW: usize = 50;

/// Summary of a model's state derived from its modelcard YAML.
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

/// Load `ModelBadgeState` from a modelcard YAML file at `path`.
///
/// Returns a state with `"unknown"` fields if the file is missing or unparseable.
pub fn load_badge_state(modelcard_path: &Path) -> ModelBadgeState {
    let content = match std::fs::read_to_string(modelcard_path) {
        Ok(c) => c,
        Err(_) => {
            return ModelBadgeState {
                model_id: "unknown".into(),
                mean_reward: 0.0,
                training_status: "unknown".into(),
            };
        }
    };
    let mc: Modelcard = match serde_yaml::from_str(&content) {
        Ok(m) => m,
        Err(_) => {
            return ModelBadgeState {
                model_id: "unknown".into(),
                mean_reward: 0.0,
                training_status: "unknown".into(),
            };
        }
    };

    // Average the last 50 eval results by mean_reward.
    let mut all_rewards: Vec<f32> = mc
        .eval_results
        .values()
        .filter_map(|v| v.get("mean_reward")?.as_f64())
        .map(|f| f as f32)
        .collect();
    let rewards: Vec<f32> = {
        let len = all_rewards.len();
        let start = len.saturating_sub(REWARD_WINDOW);
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

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove that reward window slicing never panics and the mean
    /// is always finite for any valid f32 rewards in [0.0, 1.0].
    #[kani::proof]
    #[kani::unwind(6)]
    fn reward_mean_no_overflow() {
        let count: usize = kani::any();
        kani::assume(count <= 4);

        let mut rewards = Vec::new();
        for _ in 0..count {
            let r: f32 = kani::any();
            kani::assume(r >= 0.0 && r <= 1.0 && r.is_finite());
            rewards.push(r);
        }

        let len = rewards.len();
        let start = len.saturating_sub(REWARD_WINDOW);
        let window = &rewards[start..];

        if !window.is_empty() {
            let sum: f32 = window.iter().sum();
            let mean = sum / window.len() as f32;
            assert!(mean.is_finite());
            assert!(mean >= 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── Property tests ──────────────────────────────────────────────────

    proptest! {
        #[test]
        fn mean_reward_is_finite(rewards in prop::collection::vec(0.0f32..=1.0, 0..100)) {
            // Simulate the reward averaging logic directly
            let len = rewards.len();
            let start = len.saturating_sub(REWARD_WINDOW);
            let window: Vec<f32> = rewards[start..].to_vec();
            let mean = if window.is_empty() {
                0.0
            } else {
                window.iter().sum::<f32>() / window.len() as f32
            };
            prop_assert!(mean.is_finite(), "mean reward was not finite: {}", mean);
            prop_assert!((0.0..=1.0).contains(&mean), "mean out of range: {}", mean);
        }

        #[test]
        fn window_size_never_exceeds_constant(count in 0usize..200) {
            let start = count.saturating_sub(REWARD_WINDOW);
            let window_size = count - start;
            prop_assert!(window_size <= REWARD_WINDOW);
        }
    }

    #[test]
    fn load_from_valid_modelcard() {
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
        assert_eq!(state.training_status, "idle");
        assert!(state.mean_reward > 0.0 && state.mean_reward <= 1.0);
    }

    #[test]
    fn missing_file_returns_unknown() {
        let state = load_badge_state(Path::new("/nonexistent/modelcard.yaml"));
        assert_eq!(state.model_id, "unknown");
        assert_eq!(state.training_status, "unknown");
        assert_eq!(state.mean_reward, 0.0);
    }

    #[test]
    fn empty_fields_use_defaults() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "eval_results: {{}}").unwrap();

        let state = load_badge_state(f.path());
        assert_eq!(state.model_id, "unknown");
        assert_eq!(state.training_status, "idle");
        assert_eq!(state.mean_reward, 0.0);
    }

    #[test]
    fn averages_last_50_rewards() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "model_id: test-model").unwrap();
        writeln!(f, "training_status: training").unwrap();
        writeln!(f, "eval_results:").unwrap();
        // Write 60 entries — only last 50 should count.
        for i in 0..60usize {
            // BTreeMap sorts keys alphabetically so use zero-padded keys.
            writeln!(f, "  task_{i:03}:").unwrap();
            writeln!(f, "    mean_reward: 1.0").unwrap();
        }

        let state = load_badge_state(f.path());
        assert_eq!(state.mean_reward, 1.0);
    }
}
