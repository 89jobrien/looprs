use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::file_refs::FileRefPolicy;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub file_references: FileReferencesConfig,
    pub onboarding: OnboardingConfig,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path = Path::new(".looprs/config.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        fs::create_dir_all(".looprs")?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(".looprs/config.json", content)?;
        Ok(())
    }

    pub fn set_onboarding_demo_seen(value: bool) -> anyhow::Result<()> {
        Self::set_onboarding_demo_seen_at(Path::new(".looprs/config.json"), value)
    }

    fn set_onboarding_demo_seen_at(path: &Path, value: bool) -> anyhow::Result<()> {
        use serde_json::{json, Value};

        let mut root: Value = if path.exists() {
            serde_json::from_str(&fs::read_to_string(path)?)?
        } else {
            json!({})
        };

        if !root.is_object() {
            root = json!({});
        }

        let obj = root.as_object_mut().unwrap();
        let onboarding = obj
            .entry("onboarding")
            .or_insert_with(|| json!({}));
        if !onboarding.is_object() {
            *onboarding = json!({});
        }
        onboarding
            .as_object_mut()
            .unwrap()
            .insert("demo_seen".to_string(), json!(value));

        fs::create_dir_all(".looprs")?;
        fs::write(path, serde_json::to_string_pretty(&root)?)?;
        Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OnboardingConfig {
    pub demo_seen: bool,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self { demo_seen: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn onboarding_demo_seen_defaults_false() {
        let cfg = AppConfig::default();
        assert!(!cfg.onboarding.demo_seen);
    }

    #[test]
    fn set_onboarding_demo_seen_preserves_unknown_fields() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".looprs");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.json");
        fs::write(
            &config_path,
            r#"{ "version": "1.0.0", "onboarding": {"demo_seen": false} }"#,
        )
        .unwrap();

        AppConfig::set_onboarding_demo_seen_at(&config_path, true).unwrap();

        let saved = fs::read_to_string(&config_path).unwrap();
        assert!(saved.contains("\"version\": \"1.0.0\""));
        assert!(saved.contains("\"demo_seen\": true"));
    }
}
