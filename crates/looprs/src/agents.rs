use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDefinition {
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
}

impl AgentDefinition {
    pub fn matches_prompt(&self, prompt: &str) -> bool {
        if self.triggers.is_empty() {
            return false;
        }

        let lower = prompt.to_lowercase();
        self.triggers
            .iter()
            .any(|trigger| lower.contains(&trigger.to_lowercase()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentRegistry {
    agents: HashMap<String, AgentDefinition>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Built-in agent definitions available without any `.looprs/agents/` files.
    pub fn bundled_agents() -> Vec<AgentDefinition> {
        vec![
            AgentDefinition {
                name: "reviewer".to_string(),
                role: Some("Senior Code Reviewer".to_string()),
                description: Some(
                    "Reviews code for correctness, safety, and style. \
                     Flags issues without applying fixes."
                        .to_string(),
                ),
                system_prompt: Some(
                    "You are a senior code reviewer. Read code carefully and identify \
                     bugs, logic errors, security issues, and style violations. Report \
                     findings with file and line references. Do not apply fixes."
                        .to_string(),
                ),
                tools: vec!["read".to_string(), "grep".to_string(), "glob".to_string()],
                skills: vec![],
                constraints: vec!["read-only".to_string()],
                triggers: vec![
                    "review".to_string(),
                    "inspect".to_string(),
                    "check code".to_string(),
                ],
            },
            AgentDefinition {
                name: "planner".to_string(),
                role: Some("Implementation Planner".to_string()),
                description: Some(
                    "Produces a step-by-step implementation plan. Does not write code.".to_string(),
                ),
                system_prompt: Some(
                    "You are an implementation planner. Given a task, produce a numbered, \
                     ordered plan with: goal, affected files, ordered steps, and acceptance \
                     criteria. Each step must be concrete and independently verifiable. \
                     Do not write code."
                        .to_string(),
                ),
                tools: vec!["read".to_string(), "grep".to_string(), "glob".to_string()],
                skills: vec![],
                constraints: vec!["read-only".to_string()],
                triggers: vec![
                    "plan".to_string(),
                    "design".to_string(),
                    "how should".to_string(),
                    "how to implement".to_string(),
                ],
            },
            AgentDefinition {
                name: "debugger".to_string(),
                role: Some("Systematic Debugger".to_string()),
                description: Some(
                    "Diagnoses failures by tracing root causes. \
                     Proposes minimal, targeted fixes."
                        .to_string(),
                ),
                system_prompt: Some(
                    "You are a systematic debugger. Trace failures from symptoms to root \
                     cause using evidence from logs, tests, and code. Propose the minimal \
                     change that fixes the root cause without side effects. Show reasoning."
                        .to_string(),
                ),
                tools: vec![
                    "read".to_string(),
                    "grep".to_string(),
                    "glob".to_string(),
                    "bash".to_string(),
                ],
                skills: vec![],
                constraints: vec![],
                triggers: vec![
                    "debug".to_string(),
                    "fix".to_string(),
                    "error".to_string(),
                    "failing".to_string(),
                    "broken".to_string(),
                ],
            },
        ]
    }

    pub fn register(&mut self, agent: AgentDefinition) {
        self.agents.insert(agent.name.clone(), agent);
    }

    pub fn get(&self, name: &str) -> Option<&AgentDefinition> {
        self.agents.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    pub fn list(&self) -> Vec<&AgentDefinition> {
        let mut list: Vec<&AgentDefinition> = self.agents.values().collect();
        list.sort_by_key(|a| &a.name);
        list
    }

    pub fn select_for_prompt(
        &self,
        prompt: &str,
        default_agent: Option<&str>,
        delegate_by_default: bool,
    ) -> Option<&AgentDefinition> {
        if let Some(found) = self
            .list()
            .into_iter()
            .find(|agent| agent.matches_prompt(prompt))
        {
            return Some(found);
        }

        if let Some(default_name) = default_agent
            && let Some(default) = self.get(default_name)
        {
            return Some(default);
        }

        if delegate_by_default {
            return self.list().into_iter().next();
        }

        None
    }

    // qual:allow(iosp) reason: "I/O boundary — reads agent YAML files from directory"
    pub fn load_from_directory(dir: &PathBuf) -> anyhow::Result<Self> {
        let mut registry = Self::new();

        if !dir.exists() {
            return Ok(registry);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("yaml")
                && path.extension().and_then(|s| s.to_str()) != Some("yml")
            {
                continue;
            }

            match Self::parse_agent(&path) {
                Ok(agent) => registry.register(agent),
                Err(e) => crate::ui::warn(format!(
                    "Warning: Failed to load agent {}: {}",
                    path.display(),
                    e
                )),
            }
        }

        Ok(registry)
    }

    // qual:allow(iosp) reason: "I/O boundary — loads agents from user + repo directories"
    pub fn load_dual_source(
        user_dir: Option<&PathBuf>,
        repo_dir: Option<&PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut registry = Self::new();

        if let Some(user_path) = user_dir {
            let user = Self::load_from_directory(user_path)?;
            for agent in user.list() {
                registry.register(agent.clone());
            }
        }

        if let Some(repo_path) = repo_dir {
            let repo = Self::load_from_directory(repo_path)?;
            for agent in repo.list() {
                registry.register(agent.clone());
            }
        }

        Ok(registry)
    }

    fn parse_agent(path: &std::path::Path) -> anyhow::Result<AgentDefinition> {
        let content = fs::read_to_string(path)?;
        let agent: AgentDefinition = serde_yaml::from_str(&content)?;

        if agent.name.trim().is_empty() {
            anyhow::bail!("Agent name cannot be empty");
        }

        Ok(agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn select_by_trigger_first() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentDefinition {
            name: "reviewer".to_string(),
            role: None,
            description: None,
            system_prompt: None,
            tools: vec![],
            skills: vec![],
            constraints: vec![],
            triggers: vec!["review".to_string()],
        });

        let selected = registry
            .select_for_prompt("please review this", None, false)
            .unwrap();
        assert_eq!(selected.name, "reviewer");
    }

    #[test]
    fn default_agent_applies_when_no_trigger_match() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentDefinition {
            name: "generalist".to_string(),
            role: None,
            description: None,
            system_prompt: None,
            tools: vec![],
            skills: vec![],
            constraints: vec![],
            triggers: vec![],
        });

        let selected = registry
            .select_for_prompt("hello", Some("generalist"), false)
            .unwrap();
        assert_eq!(selected.name, "generalist");
    }

    #[test]
    fn load_from_directory_parses_yaml() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("reviewer.yaml");
        let mut file = std::fs::File::create(file_path).unwrap();
        writeln!(
            file,
            r#"name: reviewer
role: Senior Reviewer
triggers:
  - review
  - lint"#
        )
        .unwrap();

        let registry = AgentRegistry::load_from_directory(&tmp.path().to_path_buf()).unwrap();
        assert!(registry.get("reviewer").is_some());
    }

    #[test]
    fn bundled_agents_returns_three_agents() {
        let agents = AgentRegistry::bundled_agents();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"reviewer"), "missing reviewer");
        assert!(names.contains(&"planner"), "missing planner");
        assert!(names.contains(&"debugger"), "missing debugger");
    }

    #[test]
    fn bundled_agents_have_required_fields() {
        for agent in AgentRegistry::bundled_agents() {
            assert!(!agent.name.is_empty());
            assert!(agent.role.is_some(), "{}: missing role", agent.name);
            assert!(
                agent.system_prompt.is_some(),
                "{}: missing system_prompt",
                agent.name
            );
            assert!(
                !agent.tools.is_empty(),
                "{}: tools must not be empty",
                agent.name
            );
            assert!(
                !agent.triggers.is_empty(),
                "{}: triggers must not be empty",
                agent.name
            );
        }
    }

    #[test]
    fn bundled_reviewer_matches_review_prompt() {
        let agents = AgentRegistry::bundled_agents();
        let reviewer = agents.iter().find(|a| a.name == "reviewer").unwrap();
        assert!(reviewer.matches_prompt("please review this PR"));
    }

    #[test]
    fn bundled_debugger_matches_error_prompt() {
        let agents = AgentRegistry::bundled_agents();
        let debugger = agents.iter().find(|a| a.name == "debugger").unwrap();
        assert!(debugger.matches_prompt("the test is failing with an error"));
    }

    #[test]
    fn bundled_planner_matches_plan_prompt() {
        let agents = AgentRegistry::bundled_agents();
        let planner = agents.iter().find(|a| a.name == "planner").unwrap();
        assert!(planner.matches_prompt("plan the refactor for this module"));
    }
}
