use anyhow::Result;
use std::env;

pub const DEFAULT_MODEL: &str = "claude-3-opus-20240229";
pub const MAX_GREP_HITS: usize = 50;
#[allow(dead_code)]
pub const API_TIMEOUT_SECS: u64 = 120;

#[derive(Clone)]
pub struct ApiConfig {
    pub key: String,
    pub url: String,
    pub model: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let (key, url, default_model) = if let Ok(key) = env::var("OPENROUTER_API_KEY") {
            (
                key,
                "https://openrouter.ai/api/v1/messages".to_string(),
                env::var("MODEL").unwrap_or_else(|_| "anthropic/claude-3-opus".to_string()),
            )
        } else if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
            (
                key,
                "https://api.anthropic.com/v1/messages".to_string(),
                env::var("MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            )
        } else {
            anyhow::bail!("No API key found. Set OPENROUTER_API_KEY or ANTHROPIC_API_KEY");
        };

        Ok(Self {
            key,
            url,
            model: default_model,
        })
    }
}

/// Get max tokens for a given model based on its context window
/// Conservative estimate: leaves 1k tokens for safety margin
pub fn get_max_tokens_for_model(model: &str) -> u32 {
    match model.to_lowercase() {
        // GPT-4 models (8k context)
        m if m.contains("gpt-4") && !m.contains("turbo") && !m.contains("32k") => 4096,
        m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") => 100000,
        m if m.contains("gpt-4-32k") => 30000,

        // GPT-5 models (128k+ context)
        m if m.contains("gpt-5") => 120000,

        // Claude models (200k context)
        m if m.contains("claude-3") || m.contains("claude-opus") => 190000,
        m if m.contains("claude") => 190000,

        // OpenRouter models (various)
        m if m.contains("anthropic") => 190000,
        m if m.contains("openai") => 100000,

        // Default to 100k for unknown models
        _ => 100000,
    }
}
