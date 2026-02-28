use freya::prelude::*;
use looprs::app_config::AppConfig;
use looprs::providers::ProviderOverrides;
use looprs::{Agent, ProviderConfig, RuntimeSettings, create_provider_with_overrides, ui};
use std::collections::HashMap;
use std::sync::OnceLock;

static SNAPSHOT: OnceLock<GuiSnapshot> = OnceLock::new();

#[derive(Clone, Default)]
struct GuiSnapshot {
    status: String,
    prompt: String,
    response: String,
}

impl GuiSnapshot {
    fn fallback() -> Self {
        Self {
            status: "No run snapshot found".to_string(),
            prompt: String::new(),
            response: "Run this binary with LOOPRS_GUI_PROMPT to execute one turn.".to_string(),
        }
    }
}

fn app() -> impl IntoElement {
    let snapshot = SNAPSHOT
        .get()
        .cloned()
        .unwrap_or_else(GuiSnapshot::fallback);

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new_all(12.0))
        .vertical()
        .child(label().text(format!("Status: {}", snapshot.status)))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .vertical()
                .child(label().text("Prompt:"))
                .child(label().text(snapshot.prompt))
                .child(label().text(""))
                .child(label().text("Response:"))
                .child(label().text(snapshot.response)),
        )
}

#[tokio::main]
async fn main() {
    ui::init_logging();

    let snapshot = run_one_turn().await;
    let _ = SNAPSHOT.set(snapshot);

    launch(LaunchConfig::new().with_window(WindowConfig::new(app)));
}

async fn run_one_turn() -> GuiSnapshot {
    let prompt = std::env::var("LOOPRS_GUI_PROMPT")
        .unwrap_or_else(|_| "Summarize what this repository does in 2 sentences.".to_string());

    let app_config = AppConfig::load().unwrap_or_default();

    let provider = match create_provider_with_overrides(ProviderOverrides { model: None }).await {
        Ok(provider) => provider,
        Err(error) => {
            return GuiSnapshot {
                status: "Provider initialization failed".to_string(),
                prompt,
                response: error.to_string(),
            };
        }
    };

    let provider_name = provider.name().to_string();
    let provider_config = ProviderConfig::load().unwrap_or_default();
    let runtime = RuntimeSettings {
        defaults: app_config.defaults.clone(),
        max_tokens_override: provider_config.merged_settings(&provider_name).max_tokens,
        fs_mode: app_config.agents.fs_mode,
    };

    let mut agent = match Agent::new_with_runtime(provider, runtime, app_config.file_ref_policy()) {
        Ok(agent) => agent,
        Err(error) => {
            return GuiSnapshot {
                status: "Agent initialization failed".to_string(),
                prompt,
                response: error.to_string(),
            };
        }
    };

    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), "gui".to_string());
    agent.set_turn_metadata(metadata);
    agent.add_user_message(prompt.clone());

    match agent.run_turn().await {
        Ok(_) => GuiSnapshot {
            status: "Turn complete".to_string(),
            prompt,
            response: agent
                .latest_assistant_text()
                .unwrap_or_else(|| "No assistant text blocks were returned.".to_string()),
        },
        Err(error) => GuiSnapshot {
            status: "Turn failed".to_string(),
            prompt,
            response: error.to_string(),
        },
    }
}
