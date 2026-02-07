use anyhow::Result;
use std::env;

pub const DEFAULT_MODEL: &str = "claude-3-opus-20240229";
pub const MAX_GREP_HITS: usize = 50;
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
