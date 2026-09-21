//! Loads provider, model-tier, and Magi settings from `~/.looprs/models.toml`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A named provider/model pair, e.g. the `"fast"` or `"judge"` tier used
/// for scoring (see [`ModelsConfig::tier`]).
#[derive(Debug, Deserialize, Clone)]
pub struct ProviderTier {
    /// Provider identifier, e.g. `"openai"` or `"anthropic"`.
    pub provider: String,
    /// Model identifier for this tier, e.g. `"gpt-4"`.
    pub model: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct MagiConfig {
    #[serde(default)]
    modelcard: String,
    #[serde(default)]
    db: String,
}

/// Parsed `~/.looprs/models.toml`: the default provider/model plus named
/// tiers (e.g. for scoring) and Magi-specific settings. Loaded via
/// [`ModelsConfig::load`] or [`ModelsConfig::from_path`].
#[derive(Debug, Deserialize, Clone)]
pub struct ModelsConfig {
    /// The provider/model used when no more specific tier applies.
    pub default: ProviderTier,
    #[serde(default)]
    tiers: HashMap<String, ProviderTier>,
    #[serde(default)]
    magi: MagiConfig,
}

impl ModelsConfig {
    /// Reads and parses a `models.toml` file at `path`.
    ///
    /// # Errors
    /// Returns an error if `path` cannot be read, or if its contents are
    /// not valid TOML matching [`ModelsConfig`]'s shape (in particular, the
    /// `[default]` table with `provider` and `model` keys is required).
    pub fn from_path(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&content).context("parsing models.toml")
    }

    /// Loads configuration from `~/.looprs/models.toml`.
    ///
    /// # Errors
    /// Returns an error if the home directory cannot be determined, or via
    /// [`ModelsConfig::from_path`] if the file is missing or invalid.
    pub fn load() -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Self::from_path(&home.join(".looprs").join("models.toml"))
    }

    /// Looks up a named tier (e.g. `"judge"`, `"fast"`), returning `None`
    /// if it isn't defined in `models.toml`.
    pub fn tier(&self, name: &str) -> Option<&ProviderTier> {
        self.tiers.get(name)
    }

    /// Returns the configured Magi modelcard path, or an empty string if
    /// the `[magi]` section or its `modelcard` key was omitted.
    pub fn magi_modelcard(&self) -> &str {
        &self.magi.modelcard
    }

    /// Returns the configured Magi database path, or an empty string if
    /// the `[magi]` section or its `db` key was omitted. An empty value is
    /// treated by callers (e.g. [`crate::agent::Agent`]) as "no database
    /// configured".
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
