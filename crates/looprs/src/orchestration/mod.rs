//! Runtime-owned prompt orchestration for agents, skills, and plugins.

use std::collections::HashMap;

use anyhow::Result;
use looprs_core::ports::{DelegatedToolPolicy, OrchestrationPluginPort};

use crate::agents::{AgentDefinition, AgentRegistry};
use crate::app_config::AgentsConfig;
use crate::plugins::manifests::PluginRuntimeRegistry;
use crate::skills::{Skill, SkillRegistry};

/// How an agent was selected for a delegated prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegationSelection {
    /// The user supplied a leading `#agent` tag.
    Explicit,
    /// An orchestration plugin selected the agent.
    Plugin,
    /// Runtime trigger/default selection chose the agent.
    Automatic,
}

impl DelegationSelection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Plugin => "plugin",
            Self::Automatic => "auto",
        }
    }
}

/// Typed capabilities and routing details for one delegated turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationContext {
    agent_name: String,
    strategy: String,
    selection: DelegationSelection,
    plugin_name: Option<String>,
    tool_policy: DelegatedToolPolicy,
}

impl DelegationContext {
    /// Construct typed routing and capability context for one delegated turn.
    pub fn new(
        agent_name: impl Into<String>,
        strategy: impl Into<String>,
        selection: DelegationSelection,
        plugin_name: Option<String>,
        tool_policy: DelegatedToolPolicy,
    ) -> Self {
        Self {
            agent_name: agent_name.into(),
            strategy: strategy.into(),
            selection,
            plugin_name,
            tool_policy,
        }
    }

    /// Selected agent identifier.
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Configured orchestration strategy.
    pub fn strategy(&self) -> &str {
        &self.strategy
    }

    /// Selection mechanism used for this turn.
    pub fn selection(&self) -> DelegationSelection {
        self.selection
    }

    /// Plugin responsible for routing, when applicable.
    pub fn plugin_name(&self) -> Option<&str> {
        self.plugin_name.as_deref()
    }

    /// Default-deny tool capability policy for this turn.
    pub fn tool_policy(&self) -> &DelegatedToolPolicy {
        &self.tool_policy
    }

    pub(crate) fn compatibility_metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::from([
            ("orchestration.mode".to_string(), "delegated".to_string()),
            ("orchestration.agent".to_string(), self.agent_name.clone()),
            ("orchestration.strategy".to_string(), self.strategy.clone()),
            (
                "orchestration.selection".to_string(),
                self.selection.as_str().to_string(),
            ),
        ]);
        if let Some(plugin_name) = &self.plugin_name {
            metadata.insert("orchestration.plugin".to_string(), plugin_name.clone());
        }
        let tools = self.tool_policy.names();
        if !tools.is_empty() {
            metadata.insert("orchestration.tools".to_string(), tools.join(","));
        }
        metadata
    }
}

/// Non-fatal routing information for presentation layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrationNotice {
    /// An explicit agent name was unknown and automatic selection was used.
    UnknownExplicitAgent(String),
}

/// Prompt and typed runtime context produced by orchestration.
#[derive(Debug, Clone)]
pub struct PreparedPrompt {
    /// Fully rendered prompt sent to the runtime agent.
    pub prompt: String,
    /// Delegation details and capabilities for the turn.
    pub delegation: Option<DelegationContext>,
    /// Skills activated while preparing the prompt.
    pub activated_skills: Vec<String>,
    /// Non-fatal routing notices for the active presentation layer.
    pub notices: Vec<OrchestrationNotice>,
}

/// Runtime service that owns agent routing, skill resolution, and prompt rendering.
pub struct RuntimeOrchestrator {
    config: AgentsConfig,
    agents: AgentRegistry,
    skills: SkillRegistry,
    plugins: PluginRuntimeRegistry,
}

impl RuntimeOrchestrator {
    /// Construct an orchestration service from loaded runtime registries.
    pub fn new(
        config: AgentsConfig,
        agents: AgentRegistry,
        skills: SkillRegistry,
        plugins: PluginRuntimeRegistry,
    ) -> Self {
        Self {
            config,
            agents,
            skills,
            plugins,
        }
    }

    /// Return loaded skill names for presentation-layer completion.
    pub fn skill_names(&self) -> Vec<String> {
        let mut names = self
            .skills
            .list()
            .into_iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    /// Prepare a prompt without automatically activating matching skills.
    pub fn prepare_prompt(&mut self, raw_prompt: &str) -> Result<PreparedPrompt> {
        self.prepare_delegation(raw_prompt, Vec::new())
    }

    /// Resolve matching skills, prepend `context_prefix`, and prepare delegation.
    pub fn prepare_message(
        &mut self,
        raw_prompt: &str,
        context_prefix: &str,
    ) -> Result<PreparedPrompt> {
        let matching = self.skills.find_matching(raw_prompt);
        let activated_skills = matching
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>();
        let resolved = render_automatic_skills(matching, raw_prompt);
        self.prepare_delegation(&format!("{context_prefix}{resolved}"), activated_skills)
    }

    /// Resolve one explicitly named skill and prepare the resulting prompt.
    pub fn prepare_skill(
        &mut self,
        skill_name: &str,
        trailing: Option<&str>,
    ) -> Result<Option<PreparedPrompt>> {
        let Some(skill) = self.skills.get(skill_name).cloned() else {
            return Ok(None);
        };
        let prompt = render_explicit_skill(&skill, trailing);
        self.prepare_delegation(&prompt, vec![skill.name]).map(Some)
    }

    fn prepare_delegation(
        &mut self,
        raw_prompt: &str,
        activated_skills: Vec<String>,
    ) -> Result<PreparedPrompt> {
        if self.agents.is_empty() {
            return Ok(PreparedPrompt {
                prompt: raw_prompt.to_string(),
                delegation: None,
                activated_skills,
                notices: Vec::new(),
            });
        }

        let mut notices = Vec::new();
        let explicit = parse_explicit_agent_tag(raw_prompt);
        let (selection, task_prompt, selection_mode, routed_by_plugin) = match explicit {
            Some((agent_name, remainder)) => {
                if let Some(agent) = self.agents.get(agent_name) {
                    (Some(agent), remainder, DelegationSelection::Explicit, None)
                } else {
                    notices.push(OrchestrationNotice::UnknownExplicitAgent(
                        agent_name.to_string(),
                    ));
                    let fallback_prompt = if remainder.is_empty() {
                        raw_prompt
                    } else {
                        remainder
                    };
                    (
                        self.auto_select(fallback_prompt),
                        fallback_prompt,
                        DelegationSelection::Automatic,
                        None,
                    )
                }
            }
            None => match self.plugins.select_agent_for_prompt(raw_prompt)? {
                Some(plugin_selection) => {
                    let manifest = self
                        .plugins
                        .orchestration_plugin(&plugin_selection.plugin_name)
                        .cloned();
                    if let Some(agent) = self.agents.get(&plugin_selection.agent_name) {
                        (
                            Some(agent),
                            raw_prompt,
                            DelegationSelection::Plugin,
                            Some(plugin_selection.plugin_name),
                        )
                    } else if manifest.as_ref().is_some_and(|manifest| manifest.required) {
                        anyhow::bail!(
                            "Required orchestration plugin '{}' routed to unknown agent '{}'",
                            plugin_selection.plugin_name,
                            plugin_selection.agent_name
                        );
                    } else {
                        (
                            self.auto_select(raw_prompt),
                            raw_prompt,
                            DelegationSelection::Automatic,
                            None,
                        )
                    }
                }
                None => (
                    self.auto_select(raw_prompt),
                    raw_prompt,
                    DelegationSelection::Automatic,
                    None,
                ),
            },
        };

        let Some(agent) = selection else {
            return Ok(PreparedPrompt {
                prompt: raw_prompt.to_string(),
                delegation: None,
                activated_skills,
                notices,
            });
        };
        let prompt = render_delegated_prompt(agent, task_prompt, &self.skills)?;
        let delegation = DelegationContext::new(
            agent.name.clone(),
            self.config.orchestration.clone(),
            selection_mode,
            routed_by_plugin,
            DelegatedToolPolicy::from_names(agent.tools.iter().cloned()),
        );
        Ok(PreparedPrompt {
            prompt,
            delegation: Some(delegation),
            activated_skills,
            notices,
        })
    }

    fn auto_select(&self, prompt: &str) -> Option<&AgentDefinition> {
        self.agents.select_for_prompt(
            prompt,
            self.config.default_agent.as_deref(),
            self.config.delegate_by_default,
        )
    }
}

fn render_automatic_skills(skills: Vec<&Skill>, raw_prompt: &str) -> String {
    if skills.is_empty() {
        return raw_prompt.to_string();
    }
    let mut prompt = skills
        .into_iter()
        .map(|skill| format!("=== Skill: {} ===\n{}\n\n", skill.name, skill.content))
        .collect::<String>();
    prompt.push_str(&format!("User message: {raw_prompt}"));
    prompt
}

fn render_explicit_skill(skill: &Skill, trailing: Option<&str>) -> String {
    match trailing {
        Some(trailing) => format!(
            "=== Skill: {} ===\n{}\n\nUser message: {}",
            skill.name, skill.content, trailing
        ),
        None => format!("Skill '{}' activated:\n\n{}", skill.name, skill.content),
    }
}

fn render_delegated_prompt(
    agent: &AgentDefinition,
    task_prompt: &str,
    skills: &SkillRegistry,
) -> Result<String> {
    let missing_skills = agent
        .skills
        .iter()
        .filter(|skill_name| skills.get(skill_name).is_none())
        .cloned()
        .collect::<Vec<_>>();
    if !missing_skills.is_empty() {
        anyhow::bail!(
            "missing delegated skill(s) for agent '{}': {}",
            agent.name,
            missing_skills.join(", ")
        );
    }
    let delegated_skills = agent
        .skills
        .iter()
        .filter_map(|skill_name| {
            skills
                .get(skill_name)
                .map(|skill| format!("- {}\n{}", skill.name, skill.content.trim_end_matches('\n')))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let skills_section = if delegated_skills.is_empty() {
        String::new()
    } else {
        format!("\nSkills:\n{delegated_skills}")
    };
    let constraints = agent
        .constraints
        .iter()
        .map(|constraint| format!("- {constraint}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!(
        "[Delegation]\nAgent: {}\nRole: {}\nDescription: {}\nSystem Prompt:\n{}\nConstraints:\n{}{}\n\nTask:\n{}",
        agent.name,
        agent.role.as_deref().unwrap_or("Specialized assistant"),
        agent.description.as_deref().unwrap_or_default(),
        agent.system_prompt.as_deref().unwrap_or_default(),
        constraints,
        skills_section,
        task_prompt
    ))
}

fn parse_explicit_agent_tag(raw_prompt: &str) -> Option<(&str, &str)> {
    let trimmed = raw_prompt.trim_start();
    let after_hash = trimmed.strip_prefix('#')?;
    if after_hash.is_empty() {
        return None;
    }
    let split_at = after_hash
        .char_indices()
        .find_map(|(index, character)| character.is_whitespace().then_some(index))
        .unwrap_or(after_hash.len());
    let agent_name = &after_hash[..split_at];
    if agent_name.is_empty()
        || !agent_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return None;
    }
    Some((agent_name, after_hash[split_at..].trim_start()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app_config::AppConfig;
    use crate::plugins::manifests::PluginRuntimeRegistry;
    use crate::{AgentDefinition, AgentRegistry, Skill, SkillRegistry};

    use super::{DelegationSelection, RuntimeOrchestrator};

    fn reviewer() -> AgentDefinition {
        AgentDefinition {
            name: "reviewer".to_string(),
            role: Some("Reviewer".to_string()),
            description: Some("Reviews code".to_string()),
            system_prompt: Some("Review for issues".to_string()),
            tools: vec!["read".to_string(), "grep".to_string()],
            skills: vec!["security-checklist".to_string()],
            constraints: vec!["read-only".to_string()],
            triggers: vec!["review".to_string()],
        }
    }

    fn orchestrator() -> RuntimeOrchestrator {
        let mut agents = AgentRegistry::new();
        agents.register(reviewer());
        let mut skills = SkillRegistry::new();
        skills.register(Skill {
            name: "security-checklist".to_string(),
            description: None,
            triggers: vec!["security".to_string()],
            content: "Check auth paths and secret handling.".to_string(),
            source_path: PathBuf::from("/tmp/security-checklist/SKILL.md"),
        });
        RuntimeOrchestrator::new(
            AppConfig::default().agents,
            agents,
            skills,
            PluginRuntimeRegistry::default(),
        )
    }

    #[test]
    fn prepares_typed_delegation_with_tool_policy_and_skills() {
        let mut service = orchestrator();

        let prepared = service
            .prepare_prompt("please review this change")
            .expect("prepare prompt");
        let delegation = prepared.delegation.expect("delegation context");

        assert_eq!(delegation.agent_name(), "reviewer");
        assert_eq!(delegation.selection(), DelegationSelection::Automatic);
        assert!(delegation.tool_policy().allows("read"));
        assert!(!delegation.tool_policy().allows("bash"));
        assert!(prepared.prompt.contains("security-checklist"));
        assert!(
            prepared
                .prompt
                .contains("Check auth paths and secret handling.")
        );
    }

    #[test]
    fn resolves_automatic_and_explicit_skill_prompts() {
        let mut service = orchestrator();

        let automatic = service
            .prepare_message("security review", "[context]\n")
            .expect("prepare automatic skill prompt");
        assert_eq!(automatic.activated_skills, vec!["security-checklist"]);
        assert!(automatic.prompt.contains("[context]"));

        let explicit = service
            .prepare_skill("security-checklist", Some("review login"))
            .expect("prepare explicit skill")
            .expect("known skill");
        assert_eq!(explicit.activated_skills, vec!["security-checklist"]);
        assert!(explicit.prompt.contains("User message: review login"));
    }

    #[test]
    fn rejects_missing_delegated_skill() {
        let mut agents = AgentRegistry::new();
        let mut agent = reviewer();
        agent.skills = vec!["missing".to_string()];
        agents.register(agent);
        let mut service = RuntimeOrchestrator::new(
            AppConfig::default().agents,
            agents,
            SkillRegistry::new(),
            PluginRuntimeRegistry::default(),
        );

        let error = service
            .prepare_prompt("please review this")
            .expect_err("missing skill must fail");
        assert!(error.to_string().contains("missing delegated skill"));
    }

    #[test]
    fn explicit_agent_tag_has_priority_and_is_removed_from_task() {
        let mut service = orchestrator();

        let prepared = service
            .prepare_prompt("#reviewer inspect login")
            .expect("prepare explicit delegation");
        let delegation = prepared.delegation.expect("delegation context");

        assert_eq!(delegation.selection(), DelegationSelection::Explicit);
        assert!(prepared.prompt.contains("Task:\ninspect login"));
        assert!(!prepared.prompt.contains("#reviewer"));
    }

    #[test]
    fn orchestration_plugin_routes_before_automatic_selection() {
        let plugins_dir = tempfile::tempdir().expect("plugin directory");
        std::fs::write(
            plugins_dir.path().join("route.yaml"),
            r#"name: route
kind: orchestration
triggers: [review]
route_to_agent: planner
"#,
        )
        .expect("write plugin");
        let mut agents = AgentRegistry::new();
        agents.register(reviewer());
        agents.register(AgentDefinition {
            name: "planner".to_string(),
            role: None,
            description: None,
            system_prompt: None,
            tools: vec![],
            skills: vec![],
            constraints: vec![],
            triggers: vec![],
        });
        let plugins =
            PluginRuntimeRegistry::load_dual_source(None, Some(plugins_dir.path().to_path_buf()))
                .expect("load plugins");
        let mut service = RuntimeOrchestrator::new(
            AppConfig::default().agents,
            agents,
            SkillRegistry::new(),
            plugins,
        );

        let prepared = service
            .prepare_prompt("review this")
            .expect("prepare plugin delegation");
        let delegation = prepared.delegation.expect("delegation context");

        assert_eq!(delegation.agent_name(), "planner");
        assert_eq!(delegation.selection(), DelegationSelection::Plugin);
        assert_eq!(delegation.plugin_name(), Some("route"));
    }
}
