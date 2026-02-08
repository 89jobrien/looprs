use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;

use looprs::{Agent, SessionContext, Event, EventContext};
use looprs::providers::create_provider;

mod cli;
use cli::{parse_input, CliCommand};

#[tokio::main]
async fn main() -> Result<()> {
    let provider = create_provider().await?;
    
    let model = provider.model().to_string();
    let provider_name = provider.name().to_string();
    
    let mut agent = Agent::new(provider)?;
    let mut rl = DefaultEditor::new()?;

    // Collect session context (jj status, bd issues, etc.)
    let context = SessionContext::collect();

    println!(
        "{} {} | {} | {}",
        ">>".bold(),
        "looprs".bold(),
        format!("{}/{}", provider_name, model).cyan(),
        env::current_dir()?.display().to_string().dimmed()
    );
    
    // Fire SessionStart event
    let session_context_str = context
        .format_for_prompt()
        .unwrap_or_default();
    let event_ctx = EventContext::new()
        .with_session_context(session_context_str);
    agent.events.fire(Event::SessionStart, &event_ctx);
    
    // Display context if available
    if !context.is_empty() {
        if let Some(formatted) = context.format_for_prompt() {
            println!("{}\n{}", "─".dimmed(), formatted.dimmed());
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

    // Fire SessionEnd event
    let event_ctx = EventContext::new();
    agent.events.fire(Event::SessionEnd, &event_ctx);

    Ok(())
}
