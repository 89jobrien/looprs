use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("No provider configured")]
    NoProviderConfigured,

    #[error("Missing API key for provider: {0}")]
    MissingApiKey(String),

    #[error("Provider configuration error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("API error: {0}")]
    ApiError(String),
}

#[derive(Debug, Error)]
pub enum ToolContextError {
    #[error("Working directory unavailable: {0}")]
    WorkingDirUnavailable(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Tool context initialization failed: {0}")]
    ToolContextInit(#[from] ToolContextError),

    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
}
