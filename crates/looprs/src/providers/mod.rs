use std::env;
use std::time::Duration;

pub mod anthropic;
pub mod anthropic_sdk;
pub mod baml_provider;
pub mod local;
pub mod openai;
pub mod openai_sdk;

use crate::api::ContentBlock;
use crate::errors::ProviderError;
use crate::types::ModelId;
use reqwest::Client;
use serde_json::{Value, json};

// Re-export the canonical inference types and trait from looprs-core.
pub use looprs_core::ports::InferenceProvider as LLMProvider;
pub use looprs_core::ports::inference_provider::{InferenceRequest, InferenceResponse, Usage};

const DEFAULT_TIMEOUT_SECS: u64 = 120;

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
    // Load .env file if available
    let _ = dotenvy::dotenv();

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

    if let Ok(provider_name) = env::var("PROVIDER") {
        return create_provider_by_name(&provider_name, &config_file, overrides).await;
    }

    if let Some(provider_name) = &config.provider {
        return create_provider_by_name(provider_name, &config_file, overrides).await;
    }

    if env::var("ANTHROPIC_API_KEY").is_ok() {
        return create_provider_by_name("anthropic", &config_file, overrides).await;
    }

    if env::var("OPENAI_API_KEY").is_ok() {
        return create_provider_by_name("openai", &config_file, overrides).await;
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
    overrides
        .model
        .clone()
        .or_else(|| env::var("MODEL").ok().map(ModelId::new))
        .or_else(|| {
            config_file
                .as_ref()
                .and_then(|c| c.merged_settings(config_section).model)
                .map(ModelId::new)
        })
}

/// Create a provider by explicit name
async fn create_provider_by_name(
    name: &str,
    config_file: &Option<crate::config_file::ProviderConfig>,
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>, ProviderError> {
    match name.to_lowercase().as_str() {
        "anthropic" => {
            let key = env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("anthropic".to_string()))?;
            let model = resolve_model("anthropic", config_file, &overrides);
            Ok(Box::new(anthropic::AnthropicProvider::new_with_model(
                key, model,
            )?))
        }
        "anthropic-sdk" | "claude-sdk" => {
            let key = env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("anthropic".to_string()))?;
            let model = resolve_model("anthropic", config_file, &overrides);
            Ok(Box::new(
                anthropic_sdk::AnthropicSdkProvider::new_with_model(key, model)?,
            ))
        }
        "openai" => {
            let key = env::var("OPENAI_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("openai".to_string()))?;
            let model = resolve_model("openai", config_file, &overrides);
            Ok(Box::new(openai::OpenAIProvider::new_with_model(
                key, model,
            )?))
        }
        "openai-sdk" => {
            let key = env::var("OPENAI_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("openai".to_string()))?;
            let model = resolve_model("openai", config_file, &overrides);
            Ok(Box::new(openai_sdk::OpenAISdkProvider::new_with_model(
                key, model,
            )?))
        }
        "ollama" | "local" => {
            let model = resolve_model("local", config_file, &overrides);
            Ok(Box::new(local::LocalProvider::new_with_model(model)?))
        }
        "baml" => {
            let model = resolve_model("anthropic", config_file, &overrides);
            Ok(Box::new(baml_provider::BamlProvider::for_provider(
                "baml", model,
            )?))
        }
        other => Err(ProviderError::Config(format!("Unknown provider: {other}"))),
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
}
