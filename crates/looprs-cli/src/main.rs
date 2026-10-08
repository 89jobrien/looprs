use anyhow::Result;
use std::env;

use looprs::ModelId;
use looprs::ui;

mod cli;
mod runtime;

pub use cli::args;
pub use cli::commands;
pub use cli::delegation;
pub use cli::extensions;
pub use cli::input;
pub use cli::interactive;
pub use cli::nu_env;
pub use cli::provider_menu;
pub use cli::repl;
pub use cli::scriptable;
pub use cli::settings;

use args::CliArgs;
use extensions::load_extensions;
use nu_env::load_nu_env;
use provider_menu::run_provider_menu;

#[tokio::main]
async fn main() -> Result<()> {
    load_nu_env();
    let args: Vec<String> = env::args().collect();
    if matches!(args.get(1).map(String::as_str), Some("provider")) {
        return run_provider_menu();
    }

    if matches!(args.get(1).map(String::as_str), Some("tui")) {
        ui::init_logging();
        let bootstrap = match runtime::bootstrap_runtime(None).await {
            Ok(bootstrap) => bootstrap,
            Err(err) => {
                if let Some(report) = runtime::provider_bootstrap_report(&err) {
                    eprintln!("{report:?}");
                    std::process::exit(1);
                }
                return Err(err);
            }
        };
        return looprs_tui::chat::run(bootstrap.agent).await;
    }

    if matches!(args.get(1).map(String::as_str), Some("seed")) {
        let dir_str = args.get(2).map(String::as_str).unwrap_or(".looprs");
        let dir = looprs::seed::expand_tilde(dir_str);
        match looprs::seed::seed_into(&dir) {
            Ok(files) => {
                for f in &files {
                    println!("{}", f.display());
                }
                std::process::exit(0);
            }
            Err(e) => {
                ui::error(format!("seed: {e}"));
                std::process::exit(1);
            }
        }
    }

    // Parse command-line arguments
    let cli_args = match CliArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            ui::error(format!("Error: {e}"));
            args::print_usage();
            std::process::exit(1);
        }
    };

    // Enable machine-readable logging if requested
    if cli_args.machine_log {
        // SAFETY: process-wide environment mutation for logging mode toggle.
        unsafe {
            std::env::set_var("LOOPRS_MACHINE_LOG", "1");
        }
    }

    ui::init_logging();

    let bootstrap = match runtime::bootstrap_runtime(cli_args.model.clone().map(ModelId::new)).await
    {
        Ok(bootstrap) => bootstrap,
        Err(err) => {
            if let Some(report) = runtime::provider_bootstrap_report(&err) {
                eprintln!("{report:?}");
                std::process::exit(1);
            }
            return Err(err);
        }
    };
    let app_config = bootstrap.app_config;
    let provider_name = bootstrap.provider_name;
    let model = bootstrap.model;
    let provider_config = bootstrap.provider_config;
    let agent = bootstrap.agent;

    let bundle = load_extensions(&cli_args, &app_config, agent);
    let agent = bundle.agent;
    let command_registry = bundle.command_registry;
    let skill_registry = bundle.skill_registry;
    let agent_registry = bundle.agent_registry;
    let plugin_runtime = bundle.plugin_runtime;

    // Handle scriptable (non-interactive) mode
    if cli_args.is_scriptable() {
        return scriptable::run_scriptable(
            &cli_args,
            &model,
            &provider_name,
            app_config,
            agent_registry,
            plugin_runtime,
            agent,
        )
        .await;
    }

    // Interactive mode
    interactive::run_interactive(
        &cli_args,
        model,
        provider_name,
        app_config,
        provider_config,
        agent,
        command_registry,
        skill_registry,
        agent_registry,
        plugin_runtime,
    )
    .await
}
