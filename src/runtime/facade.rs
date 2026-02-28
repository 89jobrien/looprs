use crate::agent::{Agent, RuntimeSettings};
use crate::app_config::AppConfig;
use crate::config_file::ProviderConfig;
use crate::providers::{ProviderOverrides, create_provider_with_overrides};
use crate::types::ModelId;

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
    let agent = Agent::new_with_runtime(provider, runtime, app_config.file_ref_policy())?;

    Ok(BootstrappedRuntime {
        app_config,
        provider_config,
        provider_name,
        model,
        agent,
    })
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
