use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Per-provider configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
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

/// Provider configuration file schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
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

impl ProviderConfig {
    /// Load config from `.looprs/provider.json`
    // qual:allow(iosp) reason: "I/O boundary — reads config file and deserializes"
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
            "anthropic" | "anthropic-sdk" | "claude-sdk" => self.anthropic.as_ref(),
            "openai" | "openai-sdk" => self.openai.as_ref(),
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
    use proptest::prelude::*;

    // ── Property tests ──────────────────────────────────────────────────

    proptest! {
        #[test]
        fn provider_config_serde_round_trip(
            provider in prop::option::of("[a-z]{3,10}"),
            model in prop::option::of("[a-z0-9-]{3,20}"),
            max_tokens in prop::option::of(1u32..100_000),
            timeout in prop::option::of(1u64..3600),
        ) {
            let config = ProviderConfig {
                provider,
                defaults: Some(ProviderSettings {
                    max_tokens,
                    timeout_secs: timeout,
                    ..Default::default()
                }),
                anthropic: Some(ProviderSettings {
                    model: model.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let json = serde_json::to_string(&config).unwrap();
            let rt: ProviderConfig = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(rt.provider, config.provider);
            prop_assert_eq!(
                rt.anthropic.as_ref().and_then(|s| s.model.as_ref()),
                config.anthropic.as_ref().and_then(|s| s.model.as_ref())
            );
            prop_assert_eq!(
                rt.defaults.as_ref().and_then(|s| s.max_tokens),
                config.defaults.as_ref().and_then(|s| s.max_tokens)
            );
        }

        #[test]
        fn merged_settings_provider_overrides_defaults(
            default_tokens in 1u32..50_000,
            provider_tokens in 1u32..50_000,
        ) {
            let config = ProviderConfig {
                defaults: Some(ProviderSettings {
                    max_tokens: Some(default_tokens),
                    ..Default::default()
                }),
                anthropic: Some(ProviderSettings {
                    max_tokens: Some(provider_tokens),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let merged = config.merged_settings("anthropic");
            prop_assert_eq!(merged.max_tokens, Some(provider_tokens));
        }

        #[test]
        fn merged_settings_falls_back_to_defaults(
            default_tokens in 1u32..50_000,
        ) {
            let config = ProviderConfig {
                defaults: Some(ProviderSettings {
                    max_tokens: Some(default_tokens),
                    ..Default::default()
                }),
                anthropic: Some(ProviderSettings::default()),
                ..Default::default()
            };
            let merged = config.merged_settings("anthropic");
            prop_assert_eq!(merged.max_tokens, Some(default_tokens));
        }

        #[test]
        fn unknown_provider_returns_defaults_only(
            name in "[a-z]{5,10}",
            default_tokens in 1u32..50_000,
        ) {
            let config = ProviderConfig {
                defaults: Some(ProviderSettings {
                    max_tokens: Some(default_tokens),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let merged = config.merged_settings(&name);
            prop_assert_eq!(merged.max_tokens, Some(default_tokens));
        }
    }

    // ── Unit tests ──────────────────────────────────────────────────────

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
