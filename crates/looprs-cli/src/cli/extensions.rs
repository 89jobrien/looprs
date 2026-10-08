use std::env;

use looprs::adapters::{ManifestRuntimeBridge, ManifestToolExecutor, PluginsAdapter};
use looprs::app_config::AppConfig;
use looprs::plugins::manifests::PluginRuntimeRegistry;
use looprs::tools::{DefaultToolExecutor, ToolExecutor};
use looprs::{Agent, AgentRegistry, CommandRegistry, HookRegistry, SkillRegistry};

use crate::args::CliArgs;

/// Everything loaded from `.looprs/` (and `~/.looprs/`) that `main()` needs
/// before dispatching to scriptable or interactive mode: hooks (folded into
/// `agent`), custom commands, skills, rules (also folded into `agent`),
/// agents, and orchestration plugins.
pub(crate) struct ExtensionBundle {
    pub(crate) agent: Agent,
    pub(crate) command_registry: CommandRegistry,
    pub(crate) skill_registry: SkillRegistry,
    pub(crate) agent_registry: AgentRegistry,
    pub(crate) plugin_runtime: PluginRuntimeRegistry,
}

/// Load hooks/commands/skills/rules/agents/plugins from both the user
/// (`~/.looprs/`) and repo (`.looprs/`) directories, repo taking precedence.
pub(crate) fn load_extensions(
    cli_args: &CliArgs,
    app_config: &AppConfig,
    mut agent: Agent,
) -> ExtensionBundle {
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

    let user_plugins_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".looprs")
        .join("plugins");
    let repo_plugins_dir = env::current_dir()
        .ok()
        .map(|d| d.join(&app_config.paths.plugins));
    let user_plugins = user_plugins_dir.exists().then_some(user_plugins_dir);
    let repo_plugins = repo_plugins_dir.filter(|d| d.exists());
    let plugin_runtime = PluginRuntimeRegistry::load_dual_source(user_plugins, repo_plugins)
        .unwrap_or_else(|_| PluginRuntimeRegistry::default());

    // Bridge PluginKind::Tool and PluginKind::Runtime manifests into the
    // agent's tool-call loop. Both `with_extra_tool_definitions` and
    // `with_tool_executor` *replace* the whole field rather than appending
    // (see agent.rs), so if both kinds are enabled at once we must compose
    // here instead of calling either setter twice — otherwise the second
    // call would silently discard the first bridge. The Runtime bridge wraps
    // the Tool bridge wraps `DefaultToolExecutor` as `inner`, mirroring how
    // `ManifestToolExecutor` itself wraps an inner `ToolExecutor`; tool
    // definitions from both kinds are merged into one vec before the single
    // `with_extra_tool_definitions` call below.
    let tool_plugins: Vec<_> = plugin_runtime
        .list_tool_plugins()
        .into_iter()
        .cloned()
        .collect();
    let runtime_plugins: Vec<_> = plugin_runtime
        .list_runtime_plugins()
        .into_iter()
        .cloned()
        .collect();

    let mut extra_tool_definitions = Vec::new();
    let mut tool_executor: Box<dyn ToolExecutor> = Box::new(DefaultToolExecutor);

    if !tool_plugins.is_empty() {
        let manifest_executor = ManifestToolExecutor::new(
            tool_executor,
            Box::new(PluginsAdapter::system()),
            tool_plugins,
        );
        extra_tool_definitions.extend(manifest_executor.tool_definitions());
        tool_executor = Box::new(manifest_executor);
    }

    if !runtime_plugins.is_empty() {
        let runtime_bridge = ManifestRuntimeBridge::new(tool_executor, runtime_plugins);
        extra_tool_definitions.extend(runtime_bridge.tool_definitions());
        tool_executor = Box::new(runtime_bridge);
    }

    if !extra_tool_definitions.is_empty() {
        agent = agent
            .with_extra_tool_definitions(extra_tool_definitions)
            .with_tool_executor(tool_executor);
    }

    ExtensionBundle {
        agent,
        command_registry,
        skill_registry,
        agent_registry,
        plugin_runtime,
    }
}
