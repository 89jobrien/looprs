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
        help(
            "Fix the listed check failures, then re-run. Set pipeline.enabled = false in .looprs/config.json to disable checks."
        )
    )]
    PipelineFailure(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of<E: Diagnostic>(err: &E) -> String {
        err.code()
            .expect("diagnostic must carry a code")
            .to_string()
    }

    #[test]
    fn provider_error_display_messages() {
        assert_eq!(
            ProviderError::NoProviderConfigured.to_string(),
            "No provider configured"
        );
        assert_eq!(
            ProviderError::MissingApiKey("anthropic".into()).to_string(),
            "Missing API key for provider: anthropic"
        );
        assert_eq!(
            ProviderError::InvalidResponse("garbled".into()).to_string(),
            "Invalid response: garbled"
        );
        assert_eq!(
            ProviderError::ApiError("rate limited".into()).to_string(),
            "API error: rate limited"
        );
    }

    #[test]
    fn provider_error_diagnostic_codes() {
        assert_eq!(
            code_of(&ProviderError::NoProviderConfigured),
            "looprs::provider::not_configured"
        );
        assert_eq!(
            code_of(&ProviderError::MissingApiKey("x".into())),
            "looprs::provider::missing_api_key"
        );
    }

    #[test]
    fn provider_error_help_text_present_for_actionable_variants() {
        assert!(ProviderError::NoProviderConfigured.help().is_some());
        assert!(ProviderError::MissingApiKey("x".into()).help().is_some());
    }

    #[test]
    fn json_error_converts_via_from() {
        let parse_err: serde_json::Error =
            serde_json::from_str::<String>("not valid json").unwrap_err();
        let converted: ProviderError = parse_err.into();
        assert!(converted.to_string().starts_with("JSON error:"));
        assert_eq!(code_of(&converted), "looprs::provider::json");
    }

    #[test]
    fn agent_error_wraps_provider_and_tool_context_errors() {
        let agent: AgentError = ProviderError::ApiError("boom".into()).into();
        assert!(matches!(
            agent,
            AgentError::Provider(ProviderError::ApiError(_))
        ));
        assert_eq!(code_of(&agent), "looprs::agent::provider");

        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let ctx_err: ToolContextError = io.into();
        let agent: AgentError = ctx_err.into();
        assert!(matches!(agent, AgentError::ToolContextInit(_)));
        assert_eq!(
            agent.to_string(),
            "Tool context initialization failed: Working directory unavailable: gone"
        );
    }

    #[test]
    fn agent_error_timeout_carries_help() {
        assert!(AgentError::Timeout.help().is_some());
        assert_eq!(
            AgentError::Timeout.to_string(),
            "Provider request timed out"
        );
    }

    #[test]
    fn tool_context_error_display_and_code() {
        let io = std::io::Error::other("cwd vanished");
        let err: ToolContextError = io.into();
        assert_eq!(
            err.to_string(),
            "Working directory unavailable: cwd vanished"
        );
        assert_eq!(code_of(&err), "looprs::tool_context::working_dir");
    }

    #[test]
    fn pipeline_failure_display_and_code() {
        let err = AgentError::PipelineFailure("fmt check failed".into());
        assert_eq!(err.to_string(), "Pipeline checks failed: fmt check failed");
        assert_eq!(code_of(&err), "looprs::agent::pipeline_failure");
    }
}
