use looprs::Agent;
use looprs::ModelId;
use looprs::ProviderConfig;
use looprs::RuntimeSettings;
use looprs::app_config::AppConfig;
use looprs::providers::{ProviderOverrides, create_provider_with_overrides};
use miette::miette;

const MISSING_LOCAL_MODEL: &str = "No local model configured";

pub struct BootstrappedRuntime {
    pub app_config: AppConfig,
    pub provider_config: ProviderConfig,
    pub provider_name: String,
    pub model: String,
    pub agent: Agent,
}

pub async fn bootstrap_runtime(
    model_override: Option<ModelId>,
) -> anyhow::Result<BootstrappedRuntime> {
    let app_config = AppConfig::load().unwrap_or_default();

    let provider = create_provider_with_overrides(ProviderOverrides {
        model: model_override,
    })
    .await?;

    let provider_name = provider.name().to_string();
    let model = provider.model().as_str().to_string();

    let provider_config = ProviderConfig::load().unwrap_or_default();
    let max_tokens_override = provider_config.merged_settings(&provider_name).max_tokens;
    let runtime = RuntimeSettings {
        defaults: app_config.defaults.clone(),
        max_tokens_override,
        fs_mode: app_config.agents.fs_mode,
    };
    let session_logger = looprs::adapters::default_session_store();
    let agent = Agent::new_with_runtime(
        provider,
        runtime,
        app_config.file_ref_policy(),
        session_logger,
        Box::new(looprs::adapters::UiOutput),
    )?;

    Ok(BootstrappedRuntime {
        app_config,
        provider_config,
        provider_name,
        model,
        agent,
    })
}

pub fn provider_bootstrap_report(error: &anyhow::Error) -> Option<miette::Report> {
    let provider_error = error.downcast_ref::<looprs::ProviderError>()?;

    match provider_error {
        looprs::ProviderError::Config(message) if message.contains(MISSING_LOCAL_MODEL) => {
            Some(miette!(
                code = "looprs::provider::missing_local_model",
                help = "Configure an Ollama model before starting looprs:\n  - Set OLLAMA_MODEL=llama3.2:latest or MODEL=llama3.2:latest\n  - Pass --model llama3.2:latest for one run\n  - Add { \"provider\": \"local\", \"local\": { \"model\": \"llama3.2:latest\" } } to .looprs/provider.json\n\nRun `ollama list` to see installed local models.",
                "No local model configured for Ollama"
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_local_model_report_explains_model_options() {
        let err = anyhow::Error::new(looprs::ProviderError::Config(
            "No local model configured. Set MODEL or OLLAMA_MODEL, or configure .looprs/provider.json"
                .to_string(),
        ));

        let report = provider_bootstrap_report(&err).expect("missing local model report");
        let rendered = format!("{report:?}");

        assert!(rendered.contains("No local model configured for Ollama"));
        assert!(rendered.contains("OLLAMA_MODEL"));
        assert!(rendered.contains("--model"));
        assert!(rendered.contains(".looprs/provider.json"));
    }

    #[test]
    fn unrelated_provider_error_does_not_get_special_report() {
        let err = anyhow::Error::new(looprs::ProviderError::MissingApiKey(
            "anthropic".to_string(),
        ));

        assert!(provider_bootstrap_report(&err).is_none());
    }
}

#[cfg(test)]
mod live_llm_tests {
    use super::*;
    use crate::runtime::events::gui_turn_metadata;
    use crate::runtime::session::run_single_turn;

    fn live_tests_enabled() -> bool {
        std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS")
            .ok()
            .is_some_and(|v| v == "1" || v == "true")
    }

    fn has_any_api_key() -> bool {
        std::env::var("ANTHROPIC_API_KEY").ok().is_some()
            || std::env::var("OPENAI_API_KEY").ok().is_some()
    }

    #[tokio::test]
    #[ignore]
    async fn live_llm_single_turn_smoke() {
        if !live_tests_enabled() || !has_any_api_key() {
            return;
        }

        let mut rt = bootstrap_runtime(None)
            .await
            .expect("bootstrap_runtime failed");

        rt.agent.clear_history();
        let response = run_single_turn(
            &mut rt.agent,
            "Reply with the single word OK.",
            gui_turn_metadata(),
        )
        .await
        .expect("run_single_turn failed");

        assert!(response.to_uppercase().contains("OK"));
    }
}
