use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

pub mod anthropic;
pub mod local;
pub mod openai;

use crate::api::{ContentBlock, Message, ToolDefinition};
use crate::types::ModelId;
use reqwest::Client;

pub(crate) struct ProviderHttpClient {
    client: Client,
}

impl ProviderHttpClient {
    pub fn new(timeout_secs: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self { client })
    }

    pub fn default() -> Result<Self> {
        Self::new(120)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// Request structure for LLM inference
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub model: ModelId,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub system: String,
}

/// Response structure from LLM inference
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,
    pub usage: Usage,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Trait for LLM providers
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Run inference with the given request
    async fn infer(&self, req: &InferenceRequest) -> Result<InferenceResponse>;

    /// Get the name of this provider
    fn name(&self) -> &str;

    /// Get the model being used
    fn model(&self) -> &ModelId;

    /// Validate that this provider is properly configured
    fn validate_config(&self) -> Result<()>;

    /// Whether this provider supports tool use (function calling)
    fn supports_tool_use(&self) -> bool {
        true
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
pub async fn create_provider() -> Result<Box<dyn LLMProvider>> {
    create_provider_with_overrides(ProviderOverrides::default()).await
}

pub async fn create_provider_with_overrides(
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>> {
    // Load .env file if available
    let _ = dotenvy::dotenv();

    // Load config file if available
    let config_file = crate::config_file::ProviderConfig::load().ok();

    // Step 1: Check explicit PROVIDER env var (highest priority)
    if let Ok(provider_name) = env::var("PROVIDER") {
        return create_provider_by_name(&provider_name, &config_file, overrides).await;
    }

    // Step 2: Check config file provider setting
    if let Some(ref config) = config_file {
        if let Some(provider_name) = &config.provider {
            return create_provider_by_name(provider_name, &config_file, overrides).await;
        }
    }

    // Step 3: Try providers in priority order based on available API keys
    if env::var("ANTHROPIC_API_KEY").is_ok() {
        let key = env::var("ANTHROPIC_API_KEY")?;
        let cfg_model = config_file
            .as_ref()
            .and_then(|c| c.merged_settings("anthropic").model)
            .map(ModelId::new);
        let model = overrides
            .model
            .or(env::var("MODEL").ok().map(ModelId::new))
            .or(cfg_model);
        return Ok(Box::new(anthropic::AnthropicProvider::new_with_model(key, model)?));
    }

    if env::var("OPENAI_API_KEY").is_ok() {
        let key = env::var("OPENAI_API_KEY")?;
        let cfg_model = config_file
            .as_ref()
            .and_then(|c| c.merged_settings("openai").model)
            .map(ModelId::new);
        let model = overrides
            .model
            .or(env::var("MODEL").ok().map(ModelId::new))
            .or(cfg_model);
        return Ok(Box::new(openai::OpenAIProvider::new_with_model(key, model)?));
    }

    // Step 4: Try local Ollama
    if local::LocalProvider::is_available().await {
        let cfg_model = config_file
            .as_ref()
            .and_then(|c| c.merged_settings("local").model)
            .map(ModelId::new);
        let model = overrides
            .model
            .or(env::var("MODEL").ok().map(ModelId::new))
            .or(cfg_model);
        return Ok(Box::new(local::LocalProvider::new_with_model(model)?));
    }

    // Step 5: Error if none found
    anyhow::bail!(
        "No LLM provider configured. Please set one of:\n\
         - ANTHROPIC_API_KEY (Anthropic)\n\
         - OPENAI_API_KEY (OpenAI)\n\
         Or run local Ollama on localhost:11434\n\
         Or create .looprs/provider.json with settings\n\
         Or set PROVIDER env var to 'local' to force Ollama"
    );
}

/// Create a provider by explicit name
async fn create_provider_by_name(
    name: &str,
    config_file: &Option<crate::config_file::ProviderConfig>,
    overrides: ProviderOverrides,
) -> Result<Box<dyn LLMProvider>> {
    match name.to_lowercase().as_str() {
        "anthropic" => {
            let key = env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("PROVIDER=anthropic but ANTHROPIC_API_KEY not set"))?;
            let cfg_model = config_file
                .as_ref()
                .and_then(|c| c.merged_settings("anthropic").model)
                .map(ModelId::new);
            let model = overrides
                .model
                .or(env::var("MODEL").ok().map(ModelId::new))
                .or(cfg_model);
            Ok(Box::new(anthropic::AnthropicProvider::new_with_model(key, model)?))
        }
        "openai" => {
            let key = env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow::anyhow!("PROVIDER=openai but OPENAI_API_KEY not set"))?;
            let cfg_model = config_file
                .as_ref()
                .and_then(|c| c.merged_settings("openai").model)
                .map(ModelId::new);
            let model = overrides
                .model
                .or(env::var("MODEL").ok().map(ModelId::new))
                .or(cfg_model);
            Ok(Box::new(openai::OpenAIProvider::new_with_model(key, model)?))
        }
        "local" | "ollama" => {
            let cfg_model = config_file
                .as_ref()
                .and_then(|c| c.merged_settings("local").model)
                .map(ModelId::new);
            let model = overrides
                .model
                .or(env::var("MODEL").ok().map(ModelId::new))
                .or(cfg_model);
            Ok(Box::new(local::LocalProvider::new_with_model(model)?))
        }
        "openrouter" => Err(anyhow::anyhow!("OpenRouter provider not yet implemented")),
        other => Err(anyhow::anyhow!("Unknown provider: {other}")),
    }
}
