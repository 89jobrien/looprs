use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderTier {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct MagiConfig {
    #[serde(default)]
    modelcard: String,
    #[serde(default)]
    db: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelsConfig {
    pub default: ProviderTier,
    #[serde(default)]
    tiers: HashMap<String, ProviderTier>,
    #[serde(default)]
    magi: MagiConfig,
}

impl ModelsConfig {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&content).context("parsing models.toml")
    }

    pub fn load() -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Self::from_path(&home.join(".looprs").join("models.toml"))
    }

    pub fn tier(&self, name: &str) -> Option<&ProviderTier> {
        self.tiers.get(name)
    }

    pub fn magi_modelcard(&self) -> &str {
        &self.magi.modelcard
    }

    pub fn magi_db(&self) -> &str {
        &self.magi.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_from_path_minimal_valid_toml() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert_eq!(config.default.provider, "openai");
        assert_eq!(config.default.model, "gpt-4");
        assert!(config.tiers.is_empty());
        assert_eq!(config.magi_modelcard(), "");
        assert_eq!(config.magi_db(), "");
    }

    #[test]
    fn test_from_path_with_tiers_and_magi() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"

[tiers.fast]
provider = "anthropic"
model = "claude-opus"

[tiers.cheap]
provider = "openai"
model = "gpt-3.5-turbo"

[magi]
modelcard = "/path/to/modelcard"
db = "/path/to/db"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert_eq!(config.default.provider, "openai");
        assert_eq!(config.default.model, "gpt-4");

        let fast_tier = config.tier("fast").expect("fast tier not found");
        assert_eq!(fast_tier.provider, "anthropic");
        assert_eq!(fast_tier.model, "claude-opus");

        let cheap_tier = config.tier("cheap").expect("cheap tier not found");
        assert_eq!(cheap_tier.provider, "openai");
        assert_eq!(cheap_tier.model, "gpt-3.5-turbo");

        assert_eq!(config.magi_modelcard(), "/path/to/modelcard");
        assert_eq!(config.magi_db(), "/path/to/db");
    }

    #[test]
    fn test_from_path_nonexistent_file() {
        let path = Path::new("/nonexistent/path/to/models.toml");
        let result = ModelsConfig::from_path(path);
        assert!(result.is_err());
        let err_msg = format!("{:?}", result);
        assert!(err_msg.contains("reading") || err_msg.contains("No such file"));
    }

    #[test]
    fn test_from_path_invalid_toml() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default
provider = "openai"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let result = ModelsConfig::from_path(file.path());
        assert!(result.is_err());
        let err_msg = format!("{:?}", result);
        assert!(err_msg.contains("parsing"));
    }

    #[test]
    fn test_tier_returns_some_for_existing() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"

[tiers.fast]
provider = "anthropic"
model = "claude-opus"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert!(config.tier("fast").is_some());
    }

    #[test]
    fn test_tier_returns_none_for_missing() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert!(config.tier("nonexistent").is_none());
    }

    #[test]
    fn test_magi_default_values_when_section_omitted() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert_eq!(config.magi_modelcard(), "");
        assert_eq!(config.magi_db(), "");
    }

    #[test]
    fn test_magi_partial_values() {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
[default]
provider = "openai"
model = "gpt-4"

[magi]
modelcard = "/path/to/modelcard"
"#;
        file.write_all(content.as_bytes())
            .expect("failed to write to temp file");
        file.flush().expect("failed to flush temp file");

        let config = ModelsConfig::from_path(file.path()).expect("failed to parse config");
        assert_eq!(config.magi_modelcard(), "/path/to/modelcard");
        assert_eq!(config.magi_db(), "");
    }
}
