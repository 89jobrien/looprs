use std::env;
use std::time::Duration;

pub mod anthropic;
pub mod anthropic_sdk;
pub mod baml_provider;
pub mod gemini;
pub mod local;
pub mod openai;
pub mod openai_sdk;
mod streaming;

use crate::api::ContentBlock;
use crate::errors::ProviderError;
use crate::types::ModelId;
use reqwest::Client;
use serde_json::{Value, json};

// Re-export the canonical inference types and trait from looprs-core.
pub use looprs_core::ports::InferenceProvider as LLMProvider;
pub use looprs_core::ports::inference_provider::{InferenceRequest, InferenceResponse, Usage};

const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Canonical identity and configuration mapping for a provider implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDescriptor {
    /// Name used after alias normalization.
    pub canonical_name: &'static str,
    /// Accepted configuration and environment aliases.
    pub aliases: &'static [&'static str],
    /// Section in `.looprs/provider.json` used by this implementation.
    pub settings_section: &'static str,
}

const PROVIDER_DESCRIPTORS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        canonical_name: "anthropic",
        aliases: &["anthropic"],
        settings_section: "anthropic",
    },
    ProviderDescriptor {
        canonical_name: "anthropic-sdk",
        aliases: &["anthropic-sdk", "claude-sdk"],
        settings_section: "anthropic",
    },
    ProviderDescriptor {
        canonical_name: "openai",
        aliases: &["openai"],
        settings_section: "openai",
    },
    ProviderDescriptor {
        canonical_name: "openai-sdk",
        aliases: &["openai-sdk"],
        settings_section: "openai",
    },
    ProviderDescriptor {
        canonical_name: "gemini",
        aliases: &["gemini", "google"],
        settings_section: "gemini",
    },
    ProviderDescriptor {
        canonical_name: "local",
        aliases: &["local", "ollama"],
        settings_section: "local",
    },
    ProviderDescriptor {
        canonical_name: "baml",
        aliases: &["baml"],
        settings_section: "baml",
    },
];

/// Resolve a provider name or alias to its canonical descriptor.
pub fn provider_descriptor(name: &str) -> Option<&'static ProviderDescriptor> {
    PROVIDER_DESCRIPTORS.iter().find(|descriptor| {
        descriptor
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(name))
    })
}

/// Returns all registered provider descriptors.
pub const fn provider_descriptors() -> &'static [ProviderDescriptor] {
    PROVIDER_DESCRIPTORS
}

/// Returns every accepted provider name and alias.
pub fn provider_aliases() -> impl Iterator<Item = &'static str> {
    PROVIDER_DESCRIPTORS
        .iter()
        .flat_map(|descriptor| descriptor.aliases.iter().copied())
}

pub(crate) struct ProviderHttpClient {
    client: Client,
}

impl ProviderHttpClient {
    pub fn new(timeout_secs: u64) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self { client })
    }

    pub fn default() -> Result<Self, ProviderError> {
        Self::new(DEFAULT_TIMEOUT_SECS)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderOverrides {
    /// Model override (e.g. from CLI -m/--model)
    pub model: Option<ModelId>,
}

/// Read an env var, resolving `op://` 1Password references via `op read`.
///
/// API key env vars are sometimes left as unresolved 1Password secret
/// references (e.g. when a shell profile doesn't source them through
/// `op run`/direnv). Rather than fail with an opaque 401, shell out to the
/// `op` CLI so looprs works the same whether the caller pre-resolved the
/// secret or not.
pub fn resolve_secret_env(var_name: &str) -> Result<String, ProviderError> {
    let value =
        env::var(var_name).map_err(|_| ProviderError::MissingApiKey(var_name.to_string()))?;

    if let Some(reference) = value.strip_prefix("op://") {
        let output = std::process::Command::new("op")
            .args(["read", &format!("op://{reference}")])
            .output()
            .map_err(|e| {
                ProviderError::Config(format!(
                    "failed to run `op read` for {var_name} ({value}): {e}"
                ))
            })?;

        if !output.status.success() {
            return Err(ProviderError::Config(format!(
                "`op read` failed for {var_name} ({value}): {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let secret = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if secret.is_empty() {
            return Err(ProviderError::Config(format!(
                "`op read` returned an empty value for {var_name} ({value})"
            )));
        }
        return Ok(secret);
    }

    Ok(value)
}

/// Check if an OpenAI model is a reasoning model (o1, o3 series).
pub(crate) fn is_reasoning_model(model: &str) -> bool {
    model.starts_with("o1") || model.starts_with("o3")
}

/// Check if an OpenAI model supports the temperature parameter.
pub(crate) fn supports_temperature(model: &str) -> bool {
    !is_reasoning_model(model) && !model.starts_with("gpt-5")
}

/// Convert a looprs Message to OpenAI-format JSON messages.
///
/// Shared by both `openai` and `openai_sdk` providers.
pub(crate) fn convert_to_openai_messages(msg: &crate::api::Message) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "id": id.as_str(),
                    "type": "function",
                    "function": {
                        "name": name.as_str(),
                        "arguments": serde_json::to_string(input).unwrap_or_default()
                    }
                }));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content: result_content,
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id.as_str(),
                    "content": result_content
                }));
            }
        }
    }

    if !text_parts.is_empty() || !tool_calls.is_empty() {
        let mut main_msg = json!({
            "role": msg.role,
        });

        if !text_parts.is_empty() {
            main_msg["content"] = json!(text_parts.join("\n"));
        } else if tool_calls.is_empty() {
            main_msg["content"] = json!("");
        }

        if !tool_calls.is_empty() {
            main_msg["tool_calls"] = json!(tool_calls);
        }

        messages.insert(0, main_msg);
    }

    messages
}

/// Create a provider based on configuration priority:
/// 1. Environment variables (highest priority)
/// 2. .looprs/provider.json config file
/// 3. Auto-detection from available API keys
/// 4. Try local Ollama
/// 5. Error if none found
pub async fn create_provider_with_overrides(
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>, ProviderError> {
    // Load config file if available
    let config_file = crate::config_file::ProviderConfig::load().ok();

    // Step 1: Check explicit PROVIDER env var (highest priority)
    if let Ok(provider_name) = env::var("PROVIDER") {
        return create_provider_by_name(&provider_name, &config_file, overrides).await;
    }

    // Step 2: Check config file provider setting
    if let Some(config) = config_file.as_ref()
        && let Some(provider_name) = &config.provider
    {
        return create_provider_by_name(provider_name, &config_file, overrides).await;
    }

    // Step 3: Try providers in priority order based on available API keys
    if env::var("ANTHROPIC_API_KEY").is_ok() {
        return create_provider_by_name("anthropic", &config_file, overrides).await;
    }

    if env::var("OPENAI_API_KEY").is_ok() {
        return create_provider_by_name("openai", &config_file, overrides).await;
    }

    if env::var("GEMINI_API_KEY").is_ok() || env::var("GOOGLE_API_KEY").is_ok() {
        return create_provider_by_name("gemini", &config_file, overrides).await;
    }

    // Step 4: Try local Ollama
    if local::LocalProvider::is_available().await {
        return create_provider_by_name("ollama", &config_file, overrides).await;
    }

    // Step 5: Error if none found
    Err(ProviderError::NoProviderConfigured)
}

/// Create a provider using an already-loaded config (for in-session switching).
///
/// Skips disk I/O. Uses the supplied `config` directly. Env vars still take
/// priority over the config's `provider` field so `PROVIDER=anthropic` wins.
pub async fn create_provider_from_config(
    config: &crate::config_file::ProviderConfig,
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>, ProviderError> {
    let config_file = Some(config.clone());

    if let Some(provider_name) = &config.provider {
        return create_provider_by_name(provider_name, &config_file, overrides).await;
    }

    if env::var("ANTHROPIC_API_KEY").is_ok() {
        return create_provider_by_name("anthropic", &config_file, overrides).await;
    }

    if env::var("OPENAI_API_KEY").is_ok() {
        return create_provider_by_name("openai", &config_file, overrides).await;
    }

    if env::var("GEMINI_API_KEY").is_ok() || env::var("GOOGLE_API_KEY").is_ok() {
        return create_provider_by_name("gemini", &config_file, overrides).await;
    }

    if local::LocalProvider::is_available().await {
        return create_provider_by_name("ollama", &config_file, overrides).await;
    }

    Err(ProviderError::NoProviderConfigured)
}

/// Resolve the effective model id from overrides, env, and config file.
fn resolve_model(
    config_section: &str,
    config_file: &Option<crate::config_file::ProviderConfig>,
    overrides: &ProviderOverrides,
) -> Option<ModelId> {
    resolve_model_from_sources(
        config_section,
        config_file.as_ref(),
        overrides,
        env::var("MODEL").ok().as_deref(),
    )
}

fn resolve_model_from_sources(
    config_section: &str,
    config_file: Option<&crate::config_file::ProviderConfig>,
    overrides: &ProviderOverrides,
    environment_model: Option<&str>,
) -> Option<ModelId> {
    overrides
        .model
        .clone()
        .and_then(normalize_model_id)
        .or_else(|| environment_model.and_then(|model| normalize_model_id(ModelId::new(model))))
        .or_else(|| {
            config_file
                .and_then(|c| c.merged_settings(config_section).model)
                .and_then(|model| normalize_model_id(ModelId::new(model)))
        })
}

fn normalize_model_id(model: ModelId) -> Option<ModelId> {
    let normalized = model.as_str().trim();
    (!normalized.is_empty()).then(|| ModelId::new(normalized))
}

/// Create a provider by explicit name
async fn create_provider_by_name(
    name: &str,
    config_file: &Option<crate::config_file::ProviderConfig>,
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>, ProviderError> {
    let descriptor = provider_descriptor(name)
        .ok_or_else(|| ProviderError::Config(format!("Unknown provider: {name}")))?;
    match descriptor.canonical_name {
        "anthropic" => {
            let key = resolve_secret_env("ANTHROPIC_API_KEY")?;
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(anthropic::AnthropicProvider::new_with_model(
                key, model,
            )?))
        }
        "anthropic-sdk" => {
            let key = resolve_secret_env("ANTHROPIC_API_KEY")?;
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(
                anthropic_sdk::AnthropicSdkProvider::new_with_model(key, model)?,
            ))
        }
        "openai" => {
            let key = resolve_secret_env("OPENAI_API_KEY")?;
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(openai::OpenAIProvider::new_with_model(
                key, model,
            )?))
        }
        "openai-sdk" => {
            let key = resolve_secret_env("OPENAI_API_KEY")?;
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(openai_sdk::OpenAISdkProvider::new_with_model(
                key, model,
            )?))
        }
        "gemini" => {
            let key = resolve_secret_env("GEMINI_API_KEY")
                .or_else(|_| resolve_secret_env("GOOGLE_API_KEY"))?;
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(gemini::GeminiProvider::new_with_model(
                key, model,
            )?))
        }
        "local" => {
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(local::LocalProvider::new_with_model(model)?))
        }
        "baml" => {
            let model = resolve_model(descriptor.settings_section, config_file, &overrides);
            Ok(Box::new(baml_provider::BamlProvider::for_provider(
                "baml", model,
            )?))
        }
        canonical => Err(ProviderError::Config(format!(
            "Provider descriptor has unsupported canonical name: {canonical}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Message;
    use looprs_core::types::{ToolId, ToolName};

    #[test]
    fn convert_to_openai_messages_text_only() {
        let msg = Message::user("hello");
        let result = convert_to_openai_messages(&msg);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "hello");
    }

    #[test]
    fn convert_to_openai_messages_tool_result() {
        let msg = Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: ToolId::new("call_1"),
            content: "output".into(),
        }]);
        let result = convert_to_openai_messages(&msg);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
        assert_eq!(result[0]["tool_call_id"], "call_1");
    }

    #[test]
    fn convert_to_openai_messages_with_tool_use() {
        let msg = Message::assistant(vec![ContentBlock::ToolUse {
            id: ToolId::new("call_2"),
            name: ToolName::new("read"),
            input: json!({"path": "foo.rs"}),
        }]);
        let result = convert_to_openai_messages(&msg);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["tool_calls"][0]["function"]["name"], "read");
    }

    #[test]
    fn is_reasoning_model_detects_o1_o3() {
        assert!(is_reasoning_model("o1-preview"));
        assert!(is_reasoning_model("o3-mini"));
        assert!(!is_reasoning_model("gpt-4o"));
    }

    #[test]
    fn supports_temperature_excludes_reasoning_and_gpt5() {
        assert!(supports_temperature("gpt-4o"));
        assert!(!supports_temperature("o1-preview"));
        assert!(!supports_temperature("gpt-5-mini"));
    }

    #[test]
    fn provider_descriptors_resolve_every_alias_to_one_settings_section() {
        for descriptor in provider_descriptors() {
            for alias in descriptor.aliases {
                assert_eq!(
                    provider_descriptor(alias),
                    Some(descriptor),
                    "alias {alias:?} did not resolve to its descriptor"
                );
            }
        }
        assert!(provider_descriptor("unknown").is_none());
        assert_eq!(
            provider_aliases().collect::<Vec<_>>(),
            vec![
                "anthropic",
                "anthropic-sdk",
                "claude-sdk",
                "openai",
                "openai-sdk",
                "gemini",
                "google",
                "local",
                "ollama",
                "baml"
            ]
        );
    }

    #[test]
    fn baml_model_resolution_respects_precedence() {
        let config = crate::config_file::ProviderConfig {
            defaults: Some(crate::config_file::ProviderSettings {
                model: Some("default-model".to_string()),
                ..Default::default()
            }),
            baml: Some(crate::config_file::ProviderSettings {
                model: Some("baml-model".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            resolve_model_from_sources("baml", Some(&config), &ProviderOverrides::default(), None,)
                .as_ref()
                .map(ModelId::as_str),
            Some("baml-model")
        );
        assert_eq!(
            resolve_model_from_sources(
                "baml",
                Some(&config),
                &ProviderOverrides::default(),
                Some("env-model"),
            )
            .as_ref()
            .map(ModelId::as_str),
            Some("env-model")
        );
        assert_eq!(
            resolve_model_from_sources(
                "baml",
                Some(&config),
                &ProviderOverrides {
                    model: Some(ModelId::new("override-model")),
                },
                Some("env-model"),
            )
            .as_ref()
            .map(ModelId::as_str),
            Some("override-model")
        );
    }

    #[test]
    fn model_resolution_normalizes_whitespace_and_ignores_blank_values() {
        assert_eq!(
            resolve_model_from_sources(
                "openai",
                None,
                &ProviderOverrides::default(),
                Some("  gpt-5-mini  "),
            )
            .as_ref()
            .map(ModelId::as_str),
            Some("gpt-5-mini")
        );
        assert!(
            resolve_model_from_sources("openai", None, &ProviderOverrides::default(), Some("   "),)
                .is_none()
        );
    }
}
