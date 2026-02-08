use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

pub mod anthropic;
pub mod local;
pub mod openai;

use crate::api::{ContentBlock, Message, ToolDefinition};

/// Request structure for LLM inference
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub model: String,
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
    async fn infer(&self, req: InferenceRequest) -> Result<InferenceResponse>;

    /// Get the name of this provider
    fn name(&self) -> &str;

    /// Get the model being used
    fn model(&self) -> &str;

    /// Validate that this provider is properly configured
    fn validate_config(&self) -> Result<()>;

    /// Whether this provider supports tool use (function calling)
    fn supports_tool_use(&self) -> bool {
        true
    }
}

/// Create a provider based on environment variables
/// Priority order:
/// 1. PROVIDER env var (explicit choice)
/// 2. ANTHROPIC_API_KEY -> Anthropic
/// 3. OPENAI_API_KEY -> OpenAI
/// 4. OPENROUTER_API_KEY -> OpenRouter (placeholder)
/// 5. Try local Ollama
pub async fn create_provider() -> Result<Box<dyn LLMProvider>> {
    // Check explicit provider choice
    if let Ok(provider_name) = env::var("PROVIDER") {
        match provider_name.to_lowercase().as_str() {
            "anthropic" => {
                let key = env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| anyhow::anyhow!("PROVIDER=anthropic but ANTHROPIC_API_KEY not set"))?;
                return Ok(Box::new(anthropic::AnthropicProvider::new(key)?));
            }
            "openai" => {
                let key = env::var("OPENAI_API_KEY")
                    .map_err(|_| anyhow::anyhow!("PROVIDER=openai but OPENAI_API_KEY not set"))?;
                return Ok(Box::new(openai::OpenAIProvider::new(key)?));
            }
            "local" | "ollama" => {
                return Ok(Box::new(local::LocalProvider::new()?));
            }
            "openrouter" => {
                anyhow::bail!("OpenRouter provider not yet implemented");
            }
            other => {
                anyhow::bail!("Unknown provider: {}", other);
            }
        }
    }

    // Try Anthropic first (most common)
    if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
        return Ok(Box::new(anthropic::AnthropicProvider::new(key)?));
    }

    // Try OpenAI
    if let Ok(key) = env::var("OPENAI_API_KEY") {
        return Ok(Box::new(openai::OpenAIProvider::new(key)?));
    }

    // Try OpenRouter
    if env::var("OPENROUTER_API_KEY").is_ok() {
        anyhow::bail!("OpenRouter provider not yet implemented. Set ANTHROPIC_API_KEY or OPENAI_API_KEY instead.");
    }

    // Try local Ollama
    if local::LocalProvider::is_available().await {
        return Ok(Box::new(local::LocalProvider::new()?));
    }

    anyhow::bail!(
        "No LLM provider configured. Please set one of:\n\
         - ANTHROPIC_API_KEY (Anthropic)\n\
         - OPENAI_API_KEY (OpenAI)\n\
         Or run local Ollama on localhost:11434\n\
         Or set PROVIDER env var to 'local' to force Ollama"
    );
}
