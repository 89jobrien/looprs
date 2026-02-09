use anyhow::Result;
use colored::*;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::env;

use looprs::observation_manager::load_recent_observations;
use looprs::providers::create_provider;
use looprs::{Agent, Event, EventContext, HookRegistry, SessionContext};

mod args;
mod cli;
use args::CliArgs;
use cli::{CliCommand, parse_input};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let cli_args = match CliArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Error: {e}");
            print_usage();
            std::process::exit(1);
        }
    };

    let provider = create_provider().await?;

    let model = provider.model().to_string();
    let provider_name = provider.name().to_string();

    let mut agent = Agent::new(provider)?;

    // Load hooks from .looprs/hooks/ directory (unless --no-hooks)
    if !cli_args.no_hooks {
        let hooks_dir = env::home_dir()
            .unwrap_or_default()
            .join(".looprs")
            .join("hooks");
        if let Ok(hooks) = HookRegistry::load_from_directory(&hooks_dir) {
            agent = agent.with_hooks(hooks);
        }
    }

    // Handle scriptable (non-interactive) mode
    if cli_args.is_scriptable() {
        return run_scriptable(&cli_args, &model, &provider_name, agent).await;
    }

    // Interactive mode
    run_interactive(&cli_args, &model, &provider_name, agent).await
}

async fn run_scriptable(
    cli_args: &CliArgs,
    model: &str,
    provider_name: &str,
    mut agent: Agent,
) -> Result<()> {
    // Get the prompt
    let Some(prompt) = cli_args.get_prompt()? else {
        eprintln!("Error: No prompt provided");
        std::process::exit(1);
    };

    // Display header unless quiet mode
    if !cli_args.quiet {
        println!(
            "{} {} | {} | {}",
            ">>".bold(),
            "looprs".bold(),
            format!("{provider_name}/{model}").cyan(),
            env::current_dir()?.display().to_string().dimmed()
        );
    }

    // Add prompt and run single turn
    agent.add_user_message(prompt);

    if let Err(e) = agent.run_turn().await {
        if cli_args.json_output {
            let error_json = serde_json::json!({
                "success": false,
                "error": e.to_string()
            });
            println!("{}", serde_json::to_string_pretty(&error_json)?);
        } else {
            eprintln!("\n{} {}", "✗".red().bold(), e.to_string().red());
        }
        std::process::exit(1);
    }

    Ok(())
}

async fn run_interactive(
    cli_args: &CliArgs,
    model: &str,
    provider_name: &str,
    mut agent: Agent,
) -> Result<()> {
    let mut rl = DefaultEditor::new()?;

    // Collect session context (jj status, bd issues, etc.)
    let context = SessionContext::collect();

    println!(
        "{} {} | {} | {}",
        ">>".bold(),
        "looprs".bold(),
        format!("{provider_name}/{model}").cyan(),
        env::current_dir()?.display().to_string().dimmed()
    );

    // Fire SessionStart event (this will also execute hooks)
    let session_context_str = context.format_for_prompt().unwrap_or_default();
    let event_ctx = EventContext::new().with_session_context(session_context_str);
    agent.events.fire(Event::SessionStart, &event_ctx);
    agent.execute_hooks_for_event(&Event::SessionStart, &event_ctx);

    // Display context if available (unless quiet mode)
    if !cli_args.quiet {
        if !context.is_empty() {
            if let Some(formatted) = context.format_for_prompt() {
                println!("{}\n{}", "─".dimmed(), formatted.dimmed());
            }
        }

        // Display recent observations if available
        if let Some(observations) = load_recent_observations(5) {
            println!("\n{}", "Recent observations:".dimmed());
            for (i, obs) in observations.iter().enumerate() {
                println!("  {} {}", (i + 1).to_string().cyan(), obs.dimmed());
            }
        }
    }

    println!("{}", "Commands: /q (quit), /c (clear history)".dimmed());

    loop {
        let readline = rl.readline(&format!("{} ", "❯".purple().bold()));

        match readline {
            Ok(line) => {
                let Some(command) = parse_input(&line) else {
                    continue;
                };

                let _ = rl.add_history_entry(&line);

                match command {
                    CliCommand::Quit => break,
                    CliCommand::Clear => {
                        agent.clear_history();
                        println!("{}", "● Conversation cleared".dimmed());
                    }
                    CliCommand::Message(msg) => {
                        agent.add_user_message(msg);

                        if let Err(e) = agent.run_turn().await {
                            eprintln!("\n{} {}", "✗".red().bold(), e.to_string().red());
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("\n{}", "Goodbye!".dimmed());
                break;
            }
            Err(e) => {
                eprintln!("Input error: {e:?}");
                break;
            }
        }
    }

    // Fire SessionEnd event and save observations
    let event_ctx = EventContext::new();
    agent.events.fire(Event::SessionEnd, &event_ctx);
    agent.execute_hooks_for_event(&Event::SessionEnd, &event_ctx);

    // Save observations to bd
    if let Err(e) = agent.observations.save_to_bd() {
        eprintln!("Warning: Failed to save observations: {e}");
    } else if agent.observations.count() > 0 {
        println!(
            "\n{} Saved {} observations to bd",
            "✓".green(),
            agent.observations.count()
        );
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        r#"Usage: looprs [OPTIONS]

OPTIONS:
  -p, --prompt <TEXT>    Run with single prompt and exit (scriptable mode)
  -f, --file <FILE>      Read prompt from file
  -m, --model <MODEL>    Override default model
  -q, --quiet            Suppress context and observations display
  --no-hooks             Skip loading hooks from ~/.looprs/hooks/
  --json                 Output response as structured JSON

EXAMPLES:
  looprs                           # Interactive mode
  looprs -p "explain closures"     # Run single prompt and exit
  looprs -f script.txt -q          # Read from file, quiet mode
  looprs -p "code" -m gpt-5.2-codex --json  # JSON output
"#
    );
}
