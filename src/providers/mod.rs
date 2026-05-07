use std::env;
use std::time::Duration;

pub mod anthropic;
pub mod anthropic_sdk;
pub mod local;
pub mod openai;
pub mod openai_sdk;

use crate::errors::ProviderError;
use crate::types::ModelId;
use reqwest::Client;

// Re-export the canonical inference types and trait from looprs-core.
pub use looprs_core::ports::InferenceProvider as LLMProvider;
pub use looprs_core::ports::inference_provider::{InferenceRequest, InferenceResponse, Usage};

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
        Self::new(120)
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

/// Create a provider based on configuration priority:
/// 1. Environment variables (highest priority)
/// 2. .looprs/provider.json config file
/// 3. Auto-detection from available API keys
/// 4. Try local Ollama
/// 5. Error if none found
pub async fn create_provider() -> Result<Box<dyn LLMProvider>, ProviderError> {
    create_provider_with_overrides(ProviderOverrides::default()).await
}

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
        return create_provider_by_name("local", &config_file, overrides).await;
    }

    // Step 5: Error if none found
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
        "local" | "ollama" => {
            let model = resolve_model("local", config_file, &overrides);
            Ok(Box::new(local::LocalProvider::new_with_model(model)?))
        }
        other => Err(ProviderError::Config(format!("Unknown provider: {other}"))),
    }
}
