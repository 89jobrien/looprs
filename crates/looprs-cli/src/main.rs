use anyhow::Result;
use colored::*;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use std::collections::HashMap;
use std::env;

use looprs::ModelId;
use looprs::app_config::AppConfig;
use looprs::file_refs::{AtReference, resolve_at_reference};
use looprs::providers::{ProviderOverrides, create_provider_with_overrides};
use looprs::ui;
use looprs::{
    Agent, AgentRegistry, ApprovalCallback, Command, CommandRegistry, Event, EventContext,
    HookRegistry, PromptCallback, SessionContext, SkillRegistry, console_approval_prompt,
    console_prompt, console_secret_prompt,
};
use looprs::{ProviderConfig, ProviderSettings};

mod args;
mod cli;
mod repl;
mod runtime;
use args::CliArgs;
use cli::{CliCommand, parse_input};
use repl::{MatchSets, ReplHelper, bind_repl_keys};

/// Walk up from `cwd` looking for `.env.nu`; source it via `nu` and inject
/// any vars it sets that aren't already in the process environment.
/// Silently no-ops if `nu` is absent or no `.env.nu` is found.
fn load_nu_env() {
    let mut dir = std::env::current_dir().unwrap_or_default();
    loop {
        let candidate = dir.join(".env.nu");
        if candidate.is_file() {
            apply_nu_env(&candidate);
            return;
        }
        if !dir.pop() {
            return;
        }
    }
}

fn apply_nu_env(path: &std::path::Path) {
    // Emit string-typed env vars as KEY=VALUE lines — avoids JSON control-char issues.
    let script = format!(
        "source '{}'; $env | items {{|k,v| if ($v | describe) == 'string' {{ $\"($k)=($v)\" }} }} | compact | str join (char newline)",
        path.display()
    );
    let Ok(output) = std::process::Command::new("nu")
        .args(["--no-config-file", "-c", &script])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Ok(text) = std::str::from_utf8(&output.stdout) else {
        return;
    };
    for line in text.lines() {
        if let Some((key, val)) = line.split_once('=')
            && std::env::var(key).is_err()
        {
            // SAFETY: single-threaded at this point in startup; no other
            // threads are reading the environment yet.
            unsafe { std::env::set_var(key, val) };
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    load_nu_env();
    let args: Vec<String> = env::args().collect();
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
            print_usage();
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
    let mut agent = bootstrap.agent;

    // Load hooks from both user (~/.looprs/hooks/) and repo (.looprs/hooks/) directories
    // Repo hooks override user hooks with same name (unless --no-hooks)
    if !cli_args.no_hooks {
        let user_hooks_dir = dirs::home_dir()
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

        if let Ok(hooks) = HookRegistry::load_dual_source(user_dir.as_ref(), repo_dir.as_ref()) {
            agent = agent.with_hooks(hooks);
        }
    }

    // Load custom commands from both user and repo directories
    let user_commands_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".looprs")
        .join("commands");

    let repo_commands_dir = env::current_dir()
        .ok()
        .map(|d| d.join(".looprs").join("commands"));

    let mut command_registry = CommandRegistry::new();

    // Load user commands
    if user_commands_dir.exists()
        && let Ok(user_commands) = CommandRegistry::load_from_directory(&user_commands_dir)
    {
        for cmd in user_commands.list() {
            command_registry.register(cmd.clone());
        }
    }

    // Load repo commands (will override user commands with same name)
    if let Some(dir) = repo_commands_dir
        && dir.exists()
        && let Ok(repo_commands) = CommandRegistry::load_from_directory(&dir)
    {
        for cmd in repo_commands.list() {
            command_registry.register(cmd.clone());
        }
    }

    // Load skills from both user and repo directories
    let user_skills_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".looprs")
        .join("skills");

    let repo_skills_dir = env::current_dir()
        .ok()
        .map(|d| d.join(".looprs").join("skills"));

    let mut skill_registry = SkillRegistry::new();

    // Load with precedence (repo overrides user)
    if let Some(repo_dir) = repo_skills_dir {
        if let Ok(_count) = skill_registry.load_with_precedence(&user_skills_dir, &repo_dir) {
            // Skills loaded successfully
        }
    } else if user_skills_dir.exists() {
        let _ = skill_registry.load_from_directory(&user_skills_dir);
    }

    // Load rules from both user and repo directories (repo overrides user)
    let rules = looprs::RuleRegistry::load_all();
    if rules.count() > 0 {
        println!("📋 Loaded {} project rule(s)", rules.count());
    }
    agent = agent.with_rules(rules);

    let user_agents_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".looprs")
        .join("agents");

    let repo_agents_dir = env::current_dir()
        .ok()
        .map(|d| d.join(&app_config.paths.agents));

    let user_agents = if user_agents_dir.exists() {
        Some(user_agents_dir)
    } else {
        None
    };
    let repo_agents = repo_agents_dir.filter(|d| d.exists());
    let agent_registry =
        AgentRegistry::load_dual_source(user_agents.as_ref(), repo_agents.as_ref())
            .unwrap_or_else(|_| AgentRegistry::new());

    // Handle scriptable (non-interactive) mode
    if cli_args.is_scriptable() {
        return run_scriptable(
            &cli_args,
            &model,
            &provider_name,
            app_config,
            agent_registry,
            agent,
        )
        .await;
    }

    // Interactive mode
    run_interactive(
        &cli_args,
        model,
        provider_name,
        app_config,
        provider_config,
        agent,
        command_registry,
        skill_registry,
        agent_registry,
    )
    .await
}

async fn run_scriptable(
    cli_args: &CliArgs,
    model: &str,
    provider_name: &str,
    app_config: AppConfig,
    agent_registry: AgentRegistry,
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
        prepare_user_prompt(&prompt, &app_config, &agent_registry);
    if !metadata.is_empty() {
        agent.set_turn_metadata(metadata);
    }
    if let Some(agent_name) = selected_agent {
        ui::info(format!("Delegated prompt to agent role: {agent_name}"));
    }
    agent.add_user_message(prepared_prompt);

    if let Err(e) = agent.run_turn().await {
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

#[allow(clippy::too_many_arguments)]
// qual:allow(iosp) reason: "CLI dispatch — interactive session entry point"
async fn run_interactive(
    cli_args: &CliArgs,
    mut model: String,
    mut provider_name: String,
    mut app_config: AppConfig,
    mut provider_config: ProviderConfig,
    mut agent: Agent,
    command_registry: CommandRegistry,
    skill_registry: SkillRegistry,
    agent_registry: AgentRegistry,
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

    // Collect session context (jj status, kan board, etc.)
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

    loop {
        let cwd_basename = env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let prompt = ui::statusline_prompt(
            &provider_name,
            &model,
            agent.fs_mode().as_str(),
            &cwd_basename,
            turn_count,
        );
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

                            let (prepared_message, metadata, selected_agent) =
                                prepare_user_prompt(&skill_message, &app_config, &agent_registry);
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

                        let (prepared_message, metadata, selected_agent) =
                            prepare_user_prompt(&final_message, &app_config, &agent_registry);
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

fn print_usage() {
    ui::error_full(
        r#"Usage: looprs [OPTIONS] | looprs seed [DIR]

COMMANDS:
  seed [DIR]             Write example config files to DIR (default: .looprs).
                         Use ~ for home (e.g. ~/.looprs). Does not overwrite.

OPTIONS:
  -p, --prompt <TEXT>    Run with single prompt and exit (scriptable mode)
  -f, --file <FILE>      Read prompt from file
  -m, --model <MODEL>    Override default model
  -q, --quiet            Suppress context and observations display
  --no-hooks             Skip loading hooks from ~/.looprs/hooks/
  --json                 Output response as structured JSON

EXAMPLES:
  looprs                           # Interactive mode
  looprs seed                      # Create .looprs/config.json.example, etc.
  looprs seed ~/.looprs            # Seed home config dir
  looprs -p "explain closures"     # Run single prompt and exit
"#,
    );
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

fn setting_keys() -> Vec<String> {
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

fn provider_settings_mut<'a>(
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

async fn handle_colon_command(
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

fn prepare_user_prompt(
    raw_prompt: &str,
    app_config: &AppConfig,
    agent_registry: &AgentRegistry,
) -> (String, HashMap<String, String>, Option<String>) {
    if agent_registry.is_empty() {
        return (raw_prompt.to_string(), HashMap::new(), None);
    }

    let selection = agent_registry.select_for_prompt(
        raw_prompt,
        app_config.agents.default_agent.as_deref(),
        app_config.agents.delegate_by_default,
    );

    let Some(agent) = selection else {
        return (raw_prompt.to_string(), HashMap::new(), None);
    };

    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), "delegated".to_string());
    metadata.insert("orchestration.agent".to_string(), agent.name.clone());
    metadata.insert(
        "orchestration.strategy".to_string(),
        app_config.agents.orchestration.clone(),
    );

    let role = agent
        .role
        .clone()
        .unwrap_or_else(|| "Specialized assistant".to_string());
    let description = agent.description.clone().unwrap_or_default();
    let system_prompt = agent.system_prompt.clone().unwrap_or_default();
    let constraints = if agent.constraints.is_empty() {
        String::new()
    } else {
        agent
            .constraints
            .iter()
            .map(|c| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let rewritten = format!(
        "[Delegation]\nAgent: {}\nRole: {}\nDescription: {}\nSystem Prompt:\n{}\nConstraints:\n{}\n\nTask:\n{}",
        agent.name, role, description, system_prompt, constraints, raw_prompt
    );

    (rewritten, metadata, Some(agent.name.clone()))
}

/// Execute a custom command
struct SessionState {
    provider_config: ProviderConfig,
    provider_name: String,
    model: String,
}

async fn execute_command(
    cmd: &Command,
    input: &str,
    agent: &mut Agent,
    app_config: &AppConfig,
    agent_registry: &AgentRegistry,
    state: &mut SessionState,
) -> Result<()> {
    let provider_config = &mut state.provider_config;
    let provider_name = &mut state.provider_name;
    let model = &mut state.model;
    use looprs::CommandAction;

    match &cmd.action {
        CommandAction::Prompt { template, .. } => {
            let (prepared_prompt, metadata, selected_agent) =
                prepare_user_prompt(template, app_config, agent_registry);
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
                ui::output_preview(trimmed);
                ui::info("Output injected into context");
                let output_prompt = format!("Command output:\n```\n{trimmed}\n```");
                let (prepared_prompt, metadata, selected_agent) =
                    prepare_user_prompt(&output_prompt, app_config, agent_registry);
                if !metadata.is_empty() {
                    agent.set_turn_metadata(metadata);
                }
                if let Some(agent_name) = selected_agent {
                    ui::info(format!("Delegated prompt to agent role: {agent_name}"));
                }
                agent.add_user_message(prepared_prompt);
            } else if !stdout.is_empty() {
                let trimmed = stdout.trim();
                ui::output_preview(trimmed);
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
                "ollama",
                "local",
                "anthropic-sdk",
                "openai-sdk",
                "claude-sdk",
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

            match looprs::providers::create_provider_from_config(
                provider_config,
                ProviderOverrides { model: None },
            )
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
    }

    Ok(())
}
