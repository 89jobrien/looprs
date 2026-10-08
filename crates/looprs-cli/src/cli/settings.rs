use anyhow::Result;

use looprs::app_config::AppConfig;
use looprs::providers::{ProviderOverrides, create_provider_with_overrides};
use looprs::ui;
use looprs::{Agent, ProviderConfig, ProviderSettings};

pub(crate) fn setting_keys() -> Vec<String> {
    vec![
        "provider",
        "model",
        "max_tokens",
        "timeout_secs",
        "defaults.max_context_tokens",
        "defaults.temperature",
        "defaults.timeout_seconds",
        "fs_mode",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

pub(crate) fn provider_settings_mut<'a>(
    config: &'a mut ProviderConfig,
    provider: &str,
) -> &'a mut ProviderSettings {
    match provider {
        "anthropic" => config
            .anthropic
            .get_or_insert_with(ProviderSettings::default),
        "openai" => config.openai.get_or_insert_with(ProviderSettings::default),
        "local" | "ollama" => config.local.get_or_insert_with(ProviderSettings::default),
        _ => config.openai.get_or_insert_with(ProviderSettings::default),
    }
}

fn provider_settings_ref<'a>(
    config: &'a ProviderConfig,
    provider: &str,
) -> Option<&'a ProviderSettings> {
    match provider {
        "anthropic" => config.anthropic.as_ref(),
        "openai" => config.openai.as_ref(),
        "local" | "ollama" => config.local.as_ref(),
        _ => None,
    }
}

fn build_runtime_settings(
    app_config: &AppConfig,
    provider_config: &ProviderConfig,
    provider_name: &str,
) -> looprs::RuntimeSettings {
    let max_tokens_override = provider_config.merged_settings(provider_name).max_tokens;
    looprs::RuntimeSettings {
        defaults: app_config.defaults.clone(),
        max_tokens_override,
        fs_mode: app_config.agents.fs_mode,
    }
}

pub(crate) async fn handle_colon_command(
    cmd: &str,
    app_config: &mut AppConfig,
    provider_config: &mut ProviderConfig,
    provider_name: &mut String,
    model: &mut String,
    agent: &mut Agent,
) -> Result<()> {
    let mut parts = cmd.split_whitespace();
    let action = parts.next().unwrap_or("");

    // Keep in-memory config in sync with live agent fs_mode (e.g. toggled via TAB).
    app_config.agents.fs_mode = agent.fs_mode();

    match action {
        "help" => {
            ui::info("Usage: :set <key> <value>, :get <key>, :unset <key>");
            ui::info("Keys: provider, model, max_tokens, timeout_secs, fs_mode, defaults.*");
        }
        "get" => {
            let key = parts.next();
            match key {
                None => {
                    let provider = provider_config
                        .provider
                        .clone()
                        .unwrap_or_else(|| "auto".to_string());
                    ui::info(format!("provider = {provider}"));
                    ui::info(format!("fs_mode = {}", agent.fs_mode().as_str()));
                    let settings = provider_settings_ref(provider_config, provider_name);
                    if let Some(settings) = settings {
                        if let Some(model) = &settings.model {
                            ui::info(format!("model = {model}"));
                        }
                        if let Some(max_tokens) = settings.max_tokens {
                            ui::info(format!("max_tokens = {max_tokens}"));
                        }
                        if let Some(timeout) = settings.timeout_secs {
                            ui::info(format!("timeout_secs = {timeout}"));
                        }
                    }
                    if let Some(v) = app_config.defaults.max_context_tokens {
                        ui::info(format!("defaults.max_context_tokens = {v}"));
                    }
                    if let Some(v) = app_config.defaults.temperature {
                        ui::info(format!("defaults.temperature = {v}"));
                    }
                    if let Some(v) = app_config.defaults.timeout_seconds {
                        ui::info(format!("defaults.timeout_seconds = {v}"));
                    }
                }
                Some(key) => {
                    if let Some(value) =
                        get_setting_value(key, app_config, provider_config, provider_name)
                    {
                        ui::info(format!("{key} = {value}"));
                    } else {
                        ui::warn(format!("Unknown setting: {key}"));
                    }
                }
            }
        }
        "unset" => {
            let key = parts.next().unwrap_or("");
            if key.is_empty() {
                ui::warn("Usage: :unset <key>");
                return Ok(());
            }
            unset_setting(key, app_config, provider_config, provider_name);
            save_configs(app_config, provider_config)?;
            let runtime = build_runtime_settings(app_config, provider_config, provider_name);
            agent.set_runtime_settings(runtime);
            agent.set_file_ref_policy(app_config.file_ref_policy());
            ui::info(format!("Unset {key}"));
        }
        "set" => {
            let key = parts.next().unwrap_or("");
            if key.is_empty() {
                ui::warn("Usage: :set <key> <value>");
                return Ok(());
            }
            let value = parts.collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                ui::warn("Usage: :set <key> <value>");
                return Ok(());
            }

            let mut reload_provider = false;
            let target_provider = provider_config
                .provider
                .clone()
                .unwrap_or_else(|| provider_name.clone());

            match key {
                "provider" => {
                    provider_config.provider = Some(value.clone());
                    reload_provider = true;
                }
                "model" => {
                    let settings = provider_settings_mut(provider_config, &target_provider);
                    settings.model = Some(value.clone());
                    reload_provider = true;
                }
                "llm" => {
                    let mut parts = value.splitn(2, '/');
                    let provider = parts.next().unwrap_or("");
                    let model = parts.next().unwrap_or("");
                    if provider.is_empty() || model.is_empty() {
                        ui::warn("Usage: :set llm <provider>/<model>");
                        return Ok(());
                    }
                    provider_config.provider = Some(provider.to_string());
                    let settings = provider_settings_mut(provider_config, provider);
                    settings.model = Some(model.to_string());
                    reload_provider = true;
                }
                "max_tokens" => {
                    let parsed = value.parse::<u32>()?;
                    let settings = provider_settings_mut(provider_config, &target_provider);
                    settings.max_tokens = Some(parsed);
                }
                "timeout_secs" => {
                    let parsed = value.parse::<u64>()?;
                    let settings = provider_settings_mut(provider_config, &target_provider);
                    settings.timeout_secs = Some(parsed);
                }
                "defaults.max_context_tokens" => {
                    app_config.defaults.max_context_tokens = Some(value.parse::<u32>()?);
                }
                "defaults.temperature" => {
                    app_config.defaults.temperature = Some(value.parse::<f32>()?);
                }
                "defaults.timeout_seconds" => {
                    app_config.defaults.timeout_seconds = Some(value.parse::<u64>()?);
                }
                _ => {
                    ui::warn(format!("Unknown setting: {key}"));
                    return Ok(());
                }
            }

            save_configs(app_config, provider_config)?;

            if reload_provider {
                let provider =
                    create_provider_with_overrides(ProviderOverrides { model: None }).await?;
                *provider_name = provider.name().to_string();
                *model = provider.model().as_str().to_string();
                agent.set_provider(provider);
                ui::info(format!("Switched to {provider_name}/{model}"));
            }

            let runtime = build_runtime_settings(app_config, provider_config, provider_name);
            agent.set_runtime_settings(runtime);
            agent.set_file_ref_policy(app_config.file_ref_policy());
            ui::info(format!("Set {key}"));
        }
        _ => {
            ui::warn(format!("Unknown command: :{action}"));
            ui::info("Try :help for available commands");
        }
    }

    Ok(())
}

fn get_setting_value(
    key: &str,
    app_config: &AppConfig,
    provider_config: &ProviderConfig,
    provider_name: &str,
) -> Option<String> {
    match key {
        "provider" => provider_config.provider.clone(),
        "model" => {
            provider_settings_ref(provider_config, provider_name).and_then(|s| s.model.clone())
        }
        "max_tokens" => provider_settings_ref(provider_config, provider_name)
            .and_then(|s| s.max_tokens)
            .map(|v| v.to_string()),
        "timeout_secs" => provider_settings_ref(provider_config, provider_name)
            .and_then(|s| s.timeout_secs)
            .map(|v| v.to_string()),
        "defaults.max_context_tokens" => app_config
            .defaults
            .max_context_tokens
            .map(|v| v.to_string()),
        "defaults.temperature" => app_config.defaults.temperature.map(|v| v.to_string()),
        "defaults.timeout_seconds" => app_config.defaults.timeout_seconds.map(|v| v.to_string()),
        "fs_mode" => Some(app_config.agents.fs_mode.as_str().to_string()),
        _ => None,
    }
}

fn unset_setting(
    key: &str,
    app_config: &mut AppConfig,
    provider_config: &mut ProviderConfig,
    provider_name: &str,
) {
    match key {
        "provider" => provider_config.provider = None,
        "model" => {
            let settings = provider_settings_mut(provider_config, provider_name);
            settings.model = None;
        }
        "max_tokens" => {
            let settings = provider_settings_mut(provider_config, provider_name);
            settings.max_tokens = None;
        }
        "timeout_secs" => {
            let settings = provider_settings_mut(provider_config, provider_name);
            settings.timeout_secs = None;
        }
        "defaults.max_context_tokens" => app_config.defaults.max_context_tokens = None,
        "defaults.temperature" => app_config.defaults.temperature = None,
        "defaults.timeout_seconds" => app_config.defaults.timeout_seconds = None,
        "fs_mode" => app_config.agents.fs_mode = looprs::FsMode::Write,
        _ => {}
    }
}

/// Config files are user-owned; we no longer write config.json or provider.json.
/// Session changes from :set/:unset apply in-memory only.
fn save_configs(_app_config: &AppConfig, _provider_config: &ProviderConfig) -> Result<()> {
    Ok(())
}
