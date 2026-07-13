use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ProviderError {
    #[error("No provider configured")]
    #[diagnostic(
        code(looprs::provider::not_configured),
        help(
            "Set PROVIDER=anthropic (or openai/local) and the matching API key env var, \
              or add a provider entry to .looprs/config.json"
        )
    )]
    NoProviderConfigured,

    #[error("Missing API key for provider: {0}")]
    #[diagnostic(
        code(looprs::provider::missing_api_key),
        help("Export the API key env var for this provider, e.g. ANTHROPIC_API_KEY=sk-…")
    )]
    MissingApiKey(String),

    #[error("Provider configuration error: {0}")]
    #[diagnostic(code(looprs::provider::config))]
    Config(String),

    #[error("HTTP error: {0}")]
    #[diagnostic(code(looprs::provider::http))]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    #[diagnostic(code(looprs::provider::json))]
    Json(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    #[diagnostic(code(looprs::provider::invalid_response))]
    InvalidResponse(String),

    #[error("API error: {0}")]
    #[diagnostic(code(looprs::provider::api))]
    ApiError(String),
}

#[derive(Debug, Error, Diagnostic)]
pub enum ToolContextError {
    #[error("Working directory unavailable: {0}")]
    #[diagnostic(
        code(looprs::tool_context::working_dir),
        help("Ensure the current working directory exists and is readable")
    )]
    WorkingDirUnavailable(#[from] std::io::Error),
}

#[derive(Debug, Error, Diagnostic)]
pub enum AgentError {
    #[error("Tool context initialization failed: {0}")]
    #[diagnostic(code(looprs::agent::tool_context_init))]
    ToolContextInit(#[from] ToolContextError),

    #[error("Provider error: {0}")]
    #[diagnostic(code(looprs::agent::provider))]
    Provider(#[from] ProviderError),

    #[error("Inference error: {0}")]
    #[diagnostic(code(looprs::agent::inference))]
    Inference(String),

    #[error("Provider request timed out")]
    #[diagnostic(
        code(looprs::agent::timeout),
        help(
            "Increase defaults.timeout_seconds in .looprs/config.json, or check network connectivity"
        )
    )]
    Timeout,

    #[error("Pipeline checks failed: {0}")]
    #[diagnostic(
        code(looprs::agent::pipeline_failure),
        help("Fix the listed check failures, then re-run. Set pipeline.enabled = false in .looprs/config.json to disable checks.")
    )]
    PipelineFailure(String),
}
