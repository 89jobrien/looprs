use anyhow::Result;
use colored::*;
use std::env;

use looprs::app_config::AppConfig;
use looprs::plugins::manifests::PluginRuntimeRegistry;
use looprs::ui;
use looprs::{Agent, AgentRegistry};

use crate::args::CliArgs;
use crate::delegation::prepare_user_prompt;

pub(crate) async fn run_scriptable(
    cli_args: &CliArgs,
    model: &str,
    provider_name: &str,
    app_config: AppConfig,
    agent_registry: AgentRegistry,
    mut plugin_runtime: PluginRuntimeRegistry,
    mut agent: Agent,
) -> Result<()> {
    // Get the prompt
    let Some(prompt) = cli_args.get_prompt()? else {
        ui::error("Error: No prompt provided");
        std::process::exit(1);
    };

    // Display header unless quiet mode
    if !cli_args.quiet {
        ui::header(
            provider_name,
            model,
            &env::current_dir()?.display().to_string(),
        );
    }

    let (prepared_prompt, metadata, selected_agent) =
        prepare_user_prompt(&prompt, &app_config, &agent_registry, &mut plugin_runtime)?;
    if !metadata.is_empty() {
        agent.set_turn_metadata(metadata);
    }
    if let Some(agent_name) = selected_agent {
        ui::info(format!("Delegated prompt to agent role: {agent_name}"));
    }
    agent.add_user_message(prepared_prompt);

    ui::assistant_lead_in();
    let result = agent.run_turn_streaming().await;
    println!();

    if let Err(e) = result {
        if cli_args.json_output {
            let error_json = serde_json::json!({
                "success": false,
                "error": e.to_string()
            });
            ui::info_full(serde_json::to_string_pretty(&error_json)?);
        } else {
            ui::error(format!("\n{} {}", "✗".red().bold(), e.to_string().red()));
        }
        std::process::exit(1);
    }

    Ok(())
}
