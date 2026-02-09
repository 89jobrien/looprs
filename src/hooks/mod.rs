use crate::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub mod executor;
pub mod parser;

pub use executor::{ApprovalCallback, HookExecutor};
pub use parser::parse_hook;

/// A hook is an event-triggered action defined in YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub name: String,
    pub trigger: String, // Event name as string (SessionStart, PostToolUse, etc.)
    pub condition: Option<String>, // Optional filter condition
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default)]
        inject_as: Option<String>,
        #[serde(default)]
        requires_approval: bool,
        #[serde(default)]
        approval_prompt: Option<String>,
    },
    #[serde(rename = "message")]
    Message { text: String },
    #[serde(rename = "conditional")]
    Conditional {
        condition: String,
        #[serde(default)]
        then: Vec<Action>,
    },
}

/// HookRegistry holds all loaded hooks keyed by event type
pub struct HookRegistry {
    hooks_by_event: HashMap<String, Vec<Hook>>,
    user_hooks: Vec<Hook>,  // Loaded from ~/.looprs/hooks/
    repo_hooks: Vec<Hook>,  // Loaded from .looprs/hooks/ (cwd)
}

impl HookRegistry {
    pub fn new() -> Self {
        HookRegistry {
            hooks_by_event: HashMap::new(),
            user_hooks: Vec::new(),
            repo_hooks: Vec::new(),
        }
    }

    pub fn load_from_directory(dir: &PathBuf) -> anyhow::Result<Self> {
        let mut registry = HookRegistry::new();

        if !dir.exists() {
            return Ok(registry); // No hooks dir, return empty registry
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match parse_hook(&path) {
                    Ok(hook) => {
                        registry
                            .hooks_by_event
                            .entry(hook.trigger.clone())
                            .or_default()
                            .push(hook);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load hook {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(registry)
    }

    /// Load hooks from both user and repo directories with precedence
    /// Repo hooks override user hooks for the same event name
    pub fn load_dual_source(user_dir: Option<&PathBuf>, repo_dir: Option<&PathBuf>) -> anyhow::Result<Self> {
        let mut registry = HookRegistry::new();

        // Load user hooks first
        if let Some(dir) = user_dir {
            registry.load_hooks_into(dir, true)?;
        }

        // Load repo hooks second (will override user hooks)
        if let Some(dir) = repo_dir {
            registry.load_hooks_into(dir, false)?;
        }

        // Merge hooks with repo precedence
        registry.merge_with_precedence();

        Ok(registry)
    }

    /// Internal helper to load hooks into user_hooks or repo_hooks
    fn load_hooks_into(&mut self, dir: &PathBuf, is_user: bool) -> anyhow::Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        let target = if is_user { &mut self.user_hooks } else { &mut self.repo_hooks };

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match parse_hook(&path) {
                    Ok(hook) => {
                        target.push(hook);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load hook {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Merge user and repo hooks with repo precedence
    /// Repo hooks with same (trigger, name) override user hooks
    fn merge_with_precedence(&mut self) {
        self.hooks_by_event.clear();

        // Add all user hooks first
        for hook in &self.user_hooks {
            self.hooks_by_event
                .entry(hook.trigger.clone())
                .or_default()
                .push(hook.clone());
        }

        // Add repo hooks - if same (trigger, name), replace user hook
        for repo_hook in &self.repo_hooks {
            let event_hooks = self.hooks_by_event
                .entry(repo_hook.trigger.clone())
                .or_default();

            // Check if user hook with same name exists
            if let Some(pos) = event_hooks.iter().position(|h| h.name == repo_hook.name) {
                // Replace user hook with repo hook
                event_hooks[pos] = repo_hook.clone();
            } else {
                // Add new repo hook
                event_hooks.push(repo_hook.clone());
            }
        }
    }

    /// Get all hooks for a specific event
    pub fn hooks_for_event(&self, event: &Event) -> Option<&Vec<Hook>> {
        let event_name = event.name();
        self.hooks_by_event.get(event_name)
    }

    /// Get hook by name
    pub fn get_hook(&self, name: &str) -> Option<&Hook> {
        for hooks in self.hooks_by_event.values() {
            for hook in hooks {
                if hook.name == name {
                    return Some(hook);
                }
            }
        }
        None
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_hook_file(dir: &std::path::Path, filename: &str, content: &str) -> anyhow::Result<()> {
        let path = dir.join(filename);
        let mut file = std::fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    #[test]
    fn test_hook_registry_empty() {
        let registry = HookRegistry::new();
        assert!(registry.hooks_by_event.is_empty());
        assert!(registry.user_hooks.is_empty());
        assert!(registry.repo_hooks.is_empty());
    }

    #[test]
    fn test_hook_registry_missing_dir() {
        let registry = HookRegistry::load_from_directory(&PathBuf::from("/nonexistent")).unwrap();
        assert!(registry.hooks_by_event.is_empty());
    }

    #[test]
    fn test_load_dual_source_user_only() {
        let user_dir = TempDir::new().unwrap();
        create_test_hook_file(
            user_dir.path(),
            "test.yaml",
            r#"name: user_hook
trigger: SessionStart
actions:
  - type: message
    text: "From user""#,
        ).unwrap();

        let registry = HookRegistry::load_dual_source(
            Some(&user_dir.path().to_path_buf()),
            None,
        ).unwrap();

        assert_eq!(registry.user_hooks.len(), 1);
        assert_eq!(registry.repo_hooks.len(), 0);
        assert_eq!(registry.hooks_by_event.get("SessionStart").unwrap().len(), 1);
        assert_eq!(registry.hooks_by_event.get("SessionStart").unwrap()[0].name, "user_hook");
    }

    #[test]
    fn test_load_dual_source_repo_only() {
        let repo_dir = TempDir::new().unwrap();
        create_test_hook_file(
            repo_dir.path(),
            "test.yaml",
            r#"name: repo_hook
trigger: SessionStart
actions:
  - type: message
    text: "From repo""#,
        ).unwrap();

        let registry = HookRegistry::load_dual_source(
            None,
            Some(&repo_dir.path().to_path_buf()),
        ).unwrap();

        assert_eq!(registry.user_hooks.len(), 0);
        assert_eq!(registry.repo_hooks.len(), 1);
        assert_eq!(registry.hooks_by_event.get("SessionStart").unwrap().len(), 1);
        assert_eq!(registry.hooks_by_event.get("SessionStart").unwrap()[0].name, "repo_hook");
    }

    #[test]
    fn test_load_dual_source_repo_overrides_user() {
        let user_dir = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();

        // User hook
        create_test_hook_file(
            user_dir.path(),
            "greeting.yaml",
            r#"name: greeting
trigger: SessionStart
actions:
  - type: message
    text: "User greeting""#,
        ).unwrap();

        // Repo hook with same name (should override)
        create_test_hook_file(
            repo_dir.path(),
            "greeting.yaml",
            r#"name: greeting
trigger: SessionStart
actions:
  - type: message
    text: "Repo greeting""#,
        ).unwrap();

        let registry = HookRegistry::load_dual_source(
            Some(&user_dir.path().to_path_buf()),
            Some(&repo_dir.path().to_path_buf()),
        ).unwrap();

        assert_eq!(registry.user_hooks.len(), 1);
        assert_eq!(registry.repo_hooks.len(), 1);
        
        // Should only have 1 hook (repo overrode user)
        let hooks = registry.hooks_by_event.get("SessionStart").unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].name, "greeting");
        
        // Verify it's the repo version by checking action text
        if let Action::Message { text } = &hooks[0].actions[0] {
            assert_eq!(text, "Repo greeting");
        } else {
            panic!("Expected Message action");
        }
    }

    #[test]
    fn test_load_dual_source_different_hooks_both_present() {
        let user_dir = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();

        // User hook
        create_test_hook_file(
            user_dir.path(),
            "user.yaml",
            r#"name: user_hook
trigger: SessionStart
actions:
  - type: message
    text: "User""#,
        ).unwrap();

        // Repo hook with different name
        create_test_hook_file(
            repo_dir.path(),
            "repo.yaml",
            r#"name: repo_hook
trigger: SessionStart
actions:
  - type: message
    text: "Repo""#,
        ).unwrap();

        let registry = HookRegistry::load_dual_source(
            Some(&user_dir.path().to_path_buf()),
            Some(&repo_dir.path().to_path_buf()),
        ).unwrap();

        assert_eq!(registry.user_hooks.len(), 1);
        assert_eq!(registry.repo_hooks.len(), 1);
        
        // Should have both hooks
        let hooks = registry.hooks_by_event.get("SessionStart").unwrap();
        assert_eq!(hooks.len(), 2);
        
        let names: Vec<&str> = hooks.iter().map(|h| h.name.as_str()).collect();
        assert!(names.contains(&"user_hook"));
        assert!(names.contains(&"repo_hook"));
    }

    #[test]
    fn test_load_dual_source_different_events() {
        let user_dir = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();

        // User hook for SessionStart
        create_test_hook_file(
            user_dir.path(),
            "start.yaml",
            r#"name: start_hook
trigger: SessionStart
actions:
  - type: message
    text: "Start""#,
        ).unwrap();

        // Repo hook for SessionEnd
        create_test_hook_file(
            repo_dir.path(),
            "end.yaml",
            r#"name: end_hook
trigger: SessionEnd
actions:
  - type: message
    text: "End""#,
        ).unwrap();

        let registry = HookRegistry::load_dual_source(
            Some(&user_dir.path().to_path_buf()),
            Some(&repo_dir.path().to_path_buf()),
        ).unwrap();

        assert_eq!(registry.hooks_by_event.len(), 2);
        assert_eq!(registry.hooks_by_event.get("SessionStart").unwrap().len(), 1);
        assert_eq!(registry.hooks_by_event.get("SessionEnd").unwrap().len(), 1);
    }
}
