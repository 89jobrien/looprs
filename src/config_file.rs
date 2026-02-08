use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Per-provider configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    /// Model name/ID (overrides MODEL env var if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Maximum tokens in response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// API timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,

    /// Custom settings per provider
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            model: None,
            max_tokens: None,
            timeout_secs: None,
            extra: Default::default(),
        }
    }
}

/// Provider configuration file schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    /// Active provider (overrides env var detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Anthropic-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<ProviderSettings>,

    /// OpenAI-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<ProviderSettings>,

    /// Local/Ollama-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<ProviderSettings>,

    /// Default settings applied to all providers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<ProviderSettings>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: None,
            anthropic: None,
            openai: None,
            local: None,
            defaults: None,
        }
    }
}

impl ProviderConfig {
    /// Load config from `.looprs/provider.json`
    pub fn load() -> Result<Self> {
        let config_path = Path::new(".looprs/provider.json");
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(config_path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save config to `.looprs/provider.json`
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(".looprs")?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(".looprs/provider.json", content)?;
        Ok(())
    }

    /// Get settings for a specific provider
    pub fn get_provider_settings(&self, provider_name: &str) -> Option<&ProviderSettings> {
        match provider_name {
            "anthropic" => self.anthropic.as_ref(),
            "openai" => self.openai.as_ref(),
            "local" | "ollama" => self.local.as_ref(),
            _ => None,
        }
    }

    /// Merge provider-specific settings with defaults
    pub fn merged_settings(&self, provider_name: &str) -> ProviderSettings {
        let mut merged = self.defaults.clone().unwrap_or_default();

        if let Some(provider_settings) = self.get_provider_settings(provider_name) {
            if let Some(model) = &provider_settings.model {
                merged.model = Some(model.clone());
            }
            if let Some(max_tokens) = provider_settings.max_tokens {
                merged.max_tokens = Some(max_tokens);
            }
            if let Some(timeout_secs) = provider_settings.timeout_secs {
                merged.timeout_secs = Some(timeout_secs);
            }
            // Merge extra settings
            for (k, v) in &provider_settings.extra {
                merged.extra.insert(k.clone(), v.clone());
            }
        }

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_merge() {
        let config = ProviderConfig {
            provider: Some("anthropic".to_string()),
            defaults: Some(ProviderSettings {
                max_tokens: Some(8192),
                ..Default::default()
            }),
            anthropic: Some(ProviderSettings {
                model: Some("claude-3-sonnet".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let merged = config.merged_settings("anthropic");
        assert_eq!(merged.model, Some("claude-3-sonnet".to_string()));
        assert_eq!(merged.max_tokens, Some(8192));
    }

    #[test]
    fn test_provider_config_serialization() {
        let config = ProviderConfig {
            provider: Some("openai".to_string()),
            openai: Some(ProviderSettings {
                model: Some("gpt-4-turbo".to_string()),
                max_tokens: Some(4096),
                ..Default::default()
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider, Some("openai".to_string()));
    }
}
