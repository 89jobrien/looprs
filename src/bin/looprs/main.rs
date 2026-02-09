use anyhow::Result;
use colored::*;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::env;

use looprs::observation_manager::load_recent_observations;
use looprs::providers::create_provider;
use looprs::{
    console_approval_prompt, Agent, ApprovalCallback, Command, CommandRegistry, Event,
    EventContext, HookRegistry, SessionContext,
};

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

    // Load hooks from both user (~/.looprs/hooks/) and repo (.looprs/hooks/) directories
    // Repo hooks override user hooks with same name (unless --no-hooks)
    if !cli_args.no_hooks {
        let user_hooks_dir = env::home_dir()
            .unwrap_or_default()
            .join(".looprs")
            .join("hooks");
        
        let repo_hooks_dir = env::current_dir()
            .ok()
            .map(|d| d.join(".looprs").join("hooks"));

        let user_dir = if user_hooks_dir.exists() {
            Some(user_hooks_dir)
        } else {
            None
        };

        let repo_dir = repo_hooks_dir.filter(|d| d.exists());

        if let Ok(hooks) = HookRegistry::load_dual_source(
            user_dir.as_ref(),
            repo_dir.as_ref(),
        ) {
            agent = agent.with_hooks(hooks);
        }
    }

    // Load custom commands from both user and repo directories
    let user_commands_dir = env::home_dir()
        .unwrap_or_default()
        .join(".looprs")
        .join("commands");
    
    let repo_commands_dir = env::current_dir()
        .ok()
        .map(|d| d.join(".looprs").join("commands"));

    let mut command_registry = CommandRegistry::new();
    
    // Load user commands
    if user_commands_dir.exists() {
        if let Ok(user_commands) = CommandRegistry::load_from_directory(&user_commands_dir) {
            for cmd in user_commands.list() {
                command_registry.register(cmd.clone());
            }
        }
    }
    
    // Load repo commands (will override user commands with same name)
    if let Some(dir) = repo_commands_dir {
        if dir.exists() {
            if let Ok(repo_commands) = CommandRegistry::load_from_directory(&dir) {
                for cmd in repo_commands.list() {
                    command_registry.register(cmd.clone());
                }
            }
        }
    }

    // Handle scriptable (non-interactive) mode
    if cli_args.is_scriptable() {
        return run_scriptable(&cli_args, &model, &provider_name, agent).await;
    }

    // Interactive mode
    run_interactive(&cli_args, &model, &provider_name, agent, command_registry).await
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
    command_registry: CommandRegistry,
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

    // Fire SessionStart event (this will also execute hooks with approval gates)
    let session_context_str = context.format_for_prompt().unwrap_or_default();
    let event_ctx = EventContext::new().with_session_context(session_context_str);
    agent.events.fire(Event::SessionStart, &event_ctx);
    
    // Create approval callback for interactive prompts
    let approval_callback: ApprovalCallback = Box::new(console_approval_prompt);
    let enriched_ctx =
        agent.execute_hooks_for_event_with_approval(&Event::SessionStart, &event_ctx, Some(&approval_callback));

    // Display context if available (unless quiet mode)
    if !cli_args.quiet {
        if !context.is_empty() {
            if let Some(formatted) = context.format_for_prompt() {
                println!("{}\n{}", "─".dimmed(), formatted.dimmed());
            }
        }

        // Display hook-injected context if available
        if !enriched_ctx.metadata.is_empty() {
            println!("\n{}", "Hook-injected context:".dimmed());
            for (key, value) in &enriched_ctx.metadata {
                let preview = if value.len() > 100 {
                    format!("{}...", &value[..100])
                } else {
                    value.clone()
                };
                println!("  {} {}", key.cyan(), preview.dimmed());
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
                    CliCommand::CustomCommand(cmd_input) => {
                        // Parse command name and args
                        let parts: Vec<&str> = cmd_input.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }
                        
                        let cmd_name = parts[0];
                        
                        if let Some(cmd) = command_registry.get(cmd_name) {
                            if let Err(e) = execute_command(cmd, &cmd_input, &mut agent).await {
                                eprintln!("{} {}", "✗".red().bold(), e.to_string().red());
                            }
                        } else {
                            println!("{} Unknown command: /{}", "✗".yellow(), cmd_name);
                            println!("Try: /help to see available commands");
                        }
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
    let _ = agent.execute_hooks_for_event(&Event::SessionEnd, &event_ctx);

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

/// Execute a custom command
async fn execute_command(cmd: &Command, _input: &str, agent: &mut Agent) -> Result<()> {
    use looprs::CommandAction;
    use std::process::Command as ProcessCommand;

    match &cmd.action {
        CommandAction::Prompt { template, .. } => {
            // Send prompt template as message to LLM
            agent.add_user_message(template);
            agent.run_turn().await?;
        }
        CommandAction::Shell {
            command,
            inject_output,
        } => {
            println!("{} Running: {}", "●".dimmed(), command.dimmed());
            let output = ProcessCommand::new("sh")
                .arg("-c")
                .arg(command)
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !output.status.success() {
                eprintln!("{stderr}");
                anyhow::bail!("Command failed with status: {}", output.status);
            }

            if *inject_output && !stdout.is_empty() {
                let trimmed = stdout.trim();
                println!("\n{trimmed}");
                println!("\n{}", "Output injected into context".dimmed());
                agent.add_user_message(format!("Command output:\n```\n{trimmed}\n```"));
            } else if !stdout.is_empty() {
                let trimmed = stdout.trim();
                println!("{trimmed}");
            }
        }
        CommandAction::Message { text } => {
            println!("{text}");
        }
    }

    Ok(())
}

