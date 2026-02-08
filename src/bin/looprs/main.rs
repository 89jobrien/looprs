use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;

use looprs::{Agent, ApiConfig};

mod cli;
use cli::{parse_input, CliCommand};

#[tokio::main]
async fn main() -> Result<()> {
    let config = ApiConfig::from_env()?;
    let mut agent = Agent::new(config.clone())?;
    let mut rl = DefaultEditor::new()?;

    println!(
        "{} {} | {} | {}",
        ">>".bold(),
        "looprs".bold(),
        config.model.cyan(),
        env::current_dir()?.display().to_string().dimmed()
    );
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
                eprintln!("Input error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
