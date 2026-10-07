use anyhow::Result;

use looprs::app_config::AppConfig;
use looprs::plugins::manifests::PluginRuntimeRegistry;
use looprs::providers::{ProviderOverrides, create_provider_from_config};
use looprs::ui;
use looprs::{Agent, AgentRegistry, Command, ProviderConfig};

use crate::delegation::prepare_user_prompt;
use crate::provider_menu::{list_ollama_models, models_gist_url};
use crate::settings::provider_settings_mut;

/// Execute a custom command
pub(crate) struct SessionState {
    pub(crate) provider_config: ProviderConfig,
    pub(crate) provider_name: String,
    pub(crate) model: String,
}

pub(crate) async fn execute_command(
    cmd: &Command,
    input: &str,
    agent: &mut Agent,
    app_config: &AppConfig,
    agent_registry: &AgentRegistry,
    plugin_runtime: &mut PluginRuntimeRegistry,
    state: &mut SessionState,
) -> Result<()> {
    let provider_config = &mut state.provider_config;
    let provider_name = &mut state.provider_name;
    let model = &mut state.model;
    use looprs::CommandAction;

    match &cmd.action {
        CommandAction::Prompt { template, .. } => {
            let (prepared_prompt, metadata, selected_agent) =
                prepare_user_prompt(template, app_config, agent_registry, plugin_runtime)?;
            if !metadata.is_empty() {
                agent.set_turn_metadata(metadata);
            }
            if let Some(agent_name) = selected_agent {
                ui::info(format!("Delegated prompt to agent role: {agent_name}"));
            }
            agent.add_user_message(prepared_prompt);
            agent.run_turn().await?;
        }
        CommandAction::Shell {
            command,
            inject_output,
        } => {
            let args = input
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            let command = command.replace("{args}", &args);
            ui::running_command(&command);
            let output = looprs::shell::run_nu_command(&command)?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !output.status.success() {
                ui::error(stderr.as_ref());
                anyhow::bail!("Command failed with status: {}", output.status);
            }

            if *inject_output && !stdout.is_empty() {
                let trimmed = stdout.trim();
                let clean = looprs::ui::output_preview_colored(trimmed);
                ui::info("Output injected into context");
                let output_prompt = format!("Command output:\n```\n{clean}\n```");
                let (prepared_prompt, metadata, selected_agent) = prepare_user_prompt(
                    &output_prompt,
                    app_config,
                    agent_registry,
                    plugin_runtime,
                )?;
                if !metadata.is_empty() {
                    agent.set_turn_metadata(metadata);
                }
                if let Some(agent_name) = selected_agent {
                    ui::info(format!("Delegated prompt to agent role: {agent_name}"));
                }
                agent.add_user_message(prepared_prompt);
            } else if !stdout.is_empty() {
                let trimmed = stdout.trim();
                looprs::ui::output_preview_colored(trimmed);
            }
        }
        CommandAction::Message { text } => {
            ui::info(text);
        }
        CommandAction::SwitchProvider => {
            // Extract args: everything after the command name
            let spec = input
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");

            if spec.is_empty() {
                // Show current provider/model
                ui::info(format!("provider: {provider_name}"));
                ui::info(format!("model:    {model}"));
                ui::info("Usage: /model <provider>[/<model-id>]");
                ui::info("  e.g. /model ollama/llama3");
                ui::info("  e.g. /model anthropic");
                return Ok(());
            }

            let mut parts = spec.splitn(2, '/');
            let new_provider = parts.next().unwrap_or("").trim().to_string();
            let new_model_id = parts.next().map(|s| s.trim().to_string());

            let valid = [
                "anthropic",
                "openai",
                "gemini",
                "google",
                "ollama",
                "local",
                "anthropic-sdk",
                "openai-sdk",
                "claude-sdk",
                "baml",
            ];
            if !valid.contains(&new_provider.as_str()) {
                ui::warn(format!(
                    "Unknown provider {new_provider:?}. Valid: {}",
                    valid.join(", ")
                ));
                return Ok(());
            }

            provider_config.provider = Some(new_provider.clone());
            if let Some(ref m) = new_model_id {
                let settings = provider_settings_mut(provider_config, &new_provider);
                settings.model = Some(m.clone());
            }

            match create_provider_from_config(provider_config, ProviderOverrides { model: None })
                .await
            {
                Ok(provider) => {
                    *provider_name = provider.name().to_string();
                    *model = provider.model().as_str().to_string();
                    agent.set_provider(provider);
                    ui::info(format!("Switched to {provider_name}/{model}"));
                }
                Err(e) => {
                    // Roll back config change on failure
                    provider_config.provider = None;
                    ui::error(format!("Failed to switch provider: {e}"));
                }
            }
        }
        CommandAction::Outsource => {
            let cfg_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".looprs/models.toml");
            match std::fs::read_to_string(&cfg_path) {
                Ok(raw) => {
                    let val: toml::Value =
                        toml::from_str(&raw).unwrap_or(toml::Value::Table(Default::default()));
                    let provider = val
                        .get("tiers")
                        .and_then(|t| t.get("outsource"))
                        .and_then(|o| o.get("provider"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let model = val
                        .get("tiers")
                        .and_then(|t| t.get("outsource"))
                        .and_then(|o| o.get("model"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    ui::info(format!(
                        "Routing to outsource provider: {provider} / {model}"
                    ));
                    ui::info("Note: this interaction will NOT be fed to magi training.");
                }
                Err(_) => ui::warn("models.toml not found at ~/.looprs/models.toml"),
            }
        }
        CommandAction::ListModels => {
            let local_models = list_ollama_models();

            let live = match looprs::model_catalog::adapters::LiveApiCatalogAdapter::new(8) {
                Ok(adapter) => adapter,
                Err(err) => {
                    ui::warn(format!("live catalog init failed: {}", err.message));
                    return Ok(());
                }
            };

            let fallback = looprs::model_catalog::adapters::PydanticAiGistCatalogAdapter::new(
                models_gist_url(),
            );

            let overview =
                looprs::build_models_overview(provider_name, model, &live, &fallback, local_models)
                    .await;
            let rendered = looprs::render_models_overview(&overview);
            ui::info_full(rendered);
        }
    }

    Ok(())
}
