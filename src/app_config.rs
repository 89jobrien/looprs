use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::file_refs::FileRefPolicy;
use crate::state::AppState;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub file_references: FileReferencesConfig,
    pub onboarding: OnboardingConfig,
}

impl AppConfig {
    /// Load from user-owned `.looprs/config.json`, then overlay onboarding from app state file.
    pub fn load() -> anyhow::Result<Self> {
        let path = Path::new(".looprs/config.json");
        let mut config: Self = if path.exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content)?
        } else {
            Self::default()
        };
        // State file (e.g. onboarding.demo_seen) overrides so app never writes config.json
        if let Ok(state) = AppState::load() {
            config.onboarding.demo_seen = state.onboarding.demo_seen;
        }
        Ok(config)
    }

    pub fn file_ref_policy(&self) -> FileRefPolicy {
        FileRefPolicy::from_config(&self.file_references)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub max_context_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub timeout_seconds: Option<u64>,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: Some(8192),
            temperature: Some(0.2),
            timeout_seconds: Some(120),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileReferencesConfig {
    pub prefix: String,
    pub max_size_mb: u64,
    pub allowed_extensions: Vec<String>,
}

impl Default for FileReferencesConfig {
    fn default() -> Self {
        Self {
            prefix: "@".to_string(),
            max_size_mb: 10,
            allowed_extensions: vec![
                "rs", "py", "ts", "js", "go", "java", "md", "txt", "json", "yaml", "toml",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OnboardingConfig {
    pub demo_seen: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn onboarding_demo_seen_defaults_false() {
        let cfg = AppConfig::default();
        assert!(!cfg.onboarding.demo_seen);
    }

    #[test]
    fn load_overlays_onboarding_from_state_file() {
        let tmp = TempDir::new().unwrap();
        let looprs = tmp.path().join(".looprs");
        std::fs::create_dir_all(&looprs).unwrap();
        std::fs::write(
            looprs.join("config.json"),
            r#"{ "onboarding": { "demo_seen": false } }"#,
        )
        .unwrap();
        std::fs::write(
            looprs.join("state.json"),
            r#"{ "onboarding": { "demo_seen": true } }"#,
        )
        .unwrap();
        let original = env::current_dir().unwrap();
        let _ = env::set_current_dir(tmp.path());
        let cfg = AppConfig::load().unwrap();
        let _ = env::set_current_dir(original);
        assert!(
            cfg.onboarding.demo_seen,
            "state file should override config"
        );
    }
}
