use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderTier {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Default)]
struct MagiConfig {
    #[serde(default)]
    modelcard: String,
    #[serde(default)]
    db: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelsConfig {
    pub default: ProviderTier,
    #[serde(default)]
    tiers: HashMap<String, ProviderTier>,
    #[serde(default)]
    magi: MagiConfig,
}

impl ModelsConfig {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&content).context("parsing models.toml")
    }

    pub fn load() -> Result<Self> {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".looprs")
            .join("models.toml");
        Self::from_path(&path)
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
