use anyhow::Result;
use colored::*;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use std::env;

use looprs::ProviderConfig;
use looprs::app_config::AppConfig;
use looprs::file_refs::{AtReference, resolve_at_reference};
use looprs::plugins::manifests::PluginRuntimeRegistry;
use looprs::ui;
use looprs::{
    Agent, AgentRegistry, ApprovalCallback, CommandRegistry, Event, EventContext, PromptCallback,
    SessionContext, SkillRegistry, console_approval_prompt, console_prompt, console_secret_prompt,
};

use crate::args::CliArgs;
use crate::commands::{SessionState, execute_command};
use crate::delegation::prepare_user_prompt;
use crate::input::{CliCommand, parse_input};
use crate::repl::{MatchSets, ReplHelper, bind_repl_keys};
use crate::settings::{handle_colon_command, setting_keys};

#[allow(clippy::too_many_arguments)]
// qual:allow(iosp) reason: "CLI dispatch — interactive session entry point"
pub(crate) async fn run_interactive(
    cli_args: &CliArgs,
    mut model: String,
    mut provider_name: String,
    mut app_config: AppConfig,
    mut provider_config: ProviderConfig,
    mut agent: Agent,
    command_registry: CommandRegistry,
    skill_registry: SkillRegistry,
    agent_registry: AgentRegistry,
    mut plugin_runtime: PluginRuntimeRegistry,
) -> Result<()> {
    let command_items = build_command_items(&command_registry);
    let skill_items = build_skill_items(&skill_registry);
    let settings_items = setting_keys();
    let helper = ReplHelper::new(MatchSets {
        commands: command_items,
        skills: skill_items,
        settings: settings_items,
    });

    let mut rl = Editor::<ReplHelper, DefaultHistory>::new()?;
    rl.set_helper(Some(helper));
    let (repl_state, repl_sets) = {
        let helper = rl.helper().expect("helper just set");
        (helper.state(), helper.sets())
    };
    bind_repl_keys(&mut rl, repl_state, repl_sets, agent.fs_mode_handle());

    // Collect session context (git status, pending doob todos, etc.)
    let context = SessionContext::collect();

    ui::header(
        &provider_name,
        &model,
        &env::current_dir()?.display().to_string(),
    );

    // Fire SessionStart event (this will also execute hooks with approval gates)
    let session_context_str = context.format_for_prompt().unwrap_or_default();
    let event_ctx = EventContext::new().with_session_context(session_context_str);
    agent.fire_event(Event::SessionStart, &event_ctx);

    // Create approval callback for interactive prompts
    let approval_callback: ApprovalCallback = Box::new(console_approval_prompt);
    let prompt_callback: PromptCallback = Box::new(console_prompt);
    let secret_prompt_callback: PromptCallback = Box::new(console_secret_prompt);
    let enriched_ctx = agent.execute_hooks_for_event_with_callbacks(
        &Event::SessionStart,
        &event_ctx,
        Some(&approval_callback),
        Some(&prompt_callback),
        Some(&secret_prompt_callback),
    );

    // Display context if available (unless quiet mode)
    if !cli_args.quiet {
        if !context.is_empty()
            && let Some(formatted) = context.format_for_prompt()
        {
            ui::info(format!("{}\n{}", "─".dimmed(), formatted.dimmed()));
        }

        // Display hook-injected context if available
        if !enriched_ctx.metadata.is_empty() {
            ui::section_title("Hook-injected context:");
            for (key, value) in &enriched_ctx.metadata {
                let preview = if value.len() > 100 {
                    format!("{}...", &value[..100])
                } else {
                    value.clone()
                };
                ui::kv_preview(key, &preview);
            }
        }
    }

    ui::info("Commands: /q (quit), /c (clear history), :set (settings)");

    let mut turn_count: usize = 0;

    let claude_statusline = env::var("LOOPRS_STATUSLINE")
        .ok()
        .is_some_and(|v| v.eq_ignore_ascii_case("statusline"));

    loop {
        let cwd_basename = env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let prompt = if claude_statusline {
            let git = looprs::git_info::collect();
            let ctx_tokens = agent.estimated_context_tokens();
            let ctx_max = agent.provider_model_max_tokens();
            let (in_tok, out_tok) = agent.session_tokens();
            let cost = agent.provider_model_id().estimate_cost(in_tok, out_tok);
            ui::statusline_prompt_statusline(&cwd_basename, &git, &model, ctx_tokens, ctx_max, cost)
        } else {
            ui::statusline_prompt(
                &provider_name,
                &model,
                agent.fs_mode().as_str(),
                &cwd_basename,
                turn_count,
            )
        };
        let readline = rl.readline(&prompt);

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
                        ui::info("● Conversation cleared");
                    }
                    CliCommand::InvokeSkill(skill_name, trailing) => {
                        if let Some(skill) = skill_registry.get(&skill_name) {
                            ui::info(format!("📚 Loading skill: {}", skill.name));
                            let skill_message = if let Some(trailing_text) = trailing {
                                let skill_message = format!(
                                    "=== Skill: {} ===\n{}\n\nUser message: {}",
                                    skill.name, skill.content, trailing_text
                                );
                                skill_message
                            } else {
                                format!("Skill '{}' activated:\n\n{}", skill.name, skill.content)
                            };

                            let (prepared_message, metadata, selected_agent) = prepare_user_prompt(
                                &skill_message,
                                &app_config,
                                &agent_registry,
                                &mut plugin_runtime,
                            )?;
                            if !metadata.is_empty() {
                                agent.set_turn_metadata(metadata);
                            }
                            if let Some(agent_name) = selected_agent {
                                ui::info(format!("Delegated prompt to agent role: {agent_name}"));
                            }

                            agent.add_user_message(prepared_message);

                            if let Err(e) = agent.run_turn().await {
                                ui::error(format!(
                                    "\n{} {}",
                                    "✗".red().bold(),
                                    e.to_string().red()
                                ));
                            }
                        } else {
                            ui::warn(format!("Skill not found: {skill_name}"));
                            ui::info("Available skills: /skills (not yet implemented)");
                        }
                    }
                    CliCommand::ColonCommand(cmd) => {
                        if let Err(e) = handle_colon_command(
                            &cmd,
                            &mut app_config,
                            &mut provider_config,
                            &mut provider_name,
                            &mut model,
                            &mut agent,
                        )
                        .await
                        {
                            ui::error(format!("{} {}", "✗".red().bold(), e.to_string().red()));
                        }
                    }
                    CliCommand::FileRef(reference) => {
                        let policy = app_config.file_ref_policy();
                        match resolve_at_reference(&reference, agent.working_dir(), &policy) {
                            Ok(AtReference::Directory(listing)) => {
                                ui::info_full(listing);
                            }
                            Ok(AtReference::File(content)) => {
                                ui::info_full(content);
                            }
                            Err(e) => {
                                ui::error(format!("{} {}", "✗".red().bold(), e.to_string().red()));
                            }
                        }
                    }
                    CliCommand::CustomCommand(cmd_input) => {
                        // Parse command name and args
                        let parts: Vec<&str> = cmd_input.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }

                        let cmd_name = parts[0];

                        if let Some(cmd) = command_registry.get(cmd_name) {
                            let mut state = SessionState {
                                provider_config: provider_config.clone(),
                                provider_name: provider_name.clone(),
                                model: model.clone(),
                            };
                            let result = execute_command(
                                cmd,
                                &cmd_input,
                                &mut agent,
                                &app_config,
                                &agent_registry,
                                &mut plugin_runtime,
                                &mut state,
                            )
                            .await;
                            provider_config = state.provider_config;
                            provider_name = state.provider_name;
                            model = state.model;
                            if let Err(e) = result {
                                ui::error(format!("{} {}", "✗".red().bold(), e.to_string().red()));
                            }
                        } else {
                            ui::warn(format!("{} Unknown command: /{}", "✗".yellow(), cmd_name));
                            ui::info("Try: /help to see available commands");
                        }
                    }
                    CliCommand::Message(msg) => {
                        // Check for auto-triggering skills
                        let matching_skills = skill_registry.find_matching(&msg);

                        let final_message = if !matching_skills.is_empty() {
                            ui::info(format!(
                                "📚 Auto-triggered {} skill(s)",
                                matching_skills.len()
                            ));
                            for skill in &matching_skills {
                                ui::info(format!("  • {}", skill.name.cyan()));
                            }

                            // Prepend skill content to user message
                            let mut full_message = String::new();
                            for skill in matching_skills {
                                full_message.push_str(&format!(
                                    "=== Skill: {} ===\n{}\n\n",
                                    skill.name, skill.content
                                ));
                            }
                            full_message.push_str(&format!("User message: {msg}"));
                            full_message
                        } else {
                            msg
                        };

                        // Inject session state so the model has current context on every turn.
                        let cwd_str = env::current_dir()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        let ctx_prefix = ui::statusline_context(
                            &provider_name,
                            &model,
                            agent.fs_mode().as_str(),
                            &cwd_str,
                            turn_count,
                        );
                        let final_message = format!("{ctx_prefix}{final_message}");

                        let (prepared_message, metadata, selected_agent) = prepare_user_prompt(
                            &final_message,
                            &app_config,
                            &agent_registry,
                            &mut plugin_runtime,
                        )?;
                        if !metadata.is_empty() {
                            agent.set_turn_metadata(metadata);
                        }
                        if let Some(agent_name) = selected_agent {
                            ui::info(format!("Delegated prompt to agent role: {agent_name}"));
                        }

                        agent.add_user_message(prepared_message);

                        if let Err(e) = agent.run_turn().await {
                            ui::error(format!("\n{} {}", "✗".red().bold(), e.to_string().red()));
                        } else {
                            turn_count += 1;
                        }
                    }
                }

                if let Some(helper) = rl.helper_mut() {
                    helper.reset();
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                ui::goodbye();
                break;
            }
            Err(e) => {
                ui::error(format!("Input error: {e:?}"));
                break;
            }
        }
    }

    // Fire SessionEnd event and save observations
    let event_ctx = EventContext::new();
    agent.fire_event(Event::SessionEnd, &event_ctx);
    let _ = agent.execute_hooks_for_event(&Event::SessionEnd, &event_ctx);

    Ok(())
}

fn build_command_items(command_registry: &CommandRegistry) -> Vec<String> {
    let mut items = Vec::new();
    for cmd in command_registry.list() {
        items.push(format!("/{}", cmd.name));
        for alias in &cmd.aliases {
            items.push(format!("/{alias}"));
        }
    }
    items.sort();
    items.dedup();
    items
}

fn build_skill_items(skill_registry: &SkillRegistry) -> Vec<String> {
    let mut items = skill_registry
        .list()
        .into_iter()
        .map(|skill| format!("${}", skill.name))
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}
