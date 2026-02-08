use crate::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub mod executor;
pub mod parser;

pub use executor::HookExecutor;
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
}

impl HookRegistry {
    pub fn new() -> Self {
        HookRegistry {
            hooks_by_event: HashMap::new(),
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
                            .or_insert_with(Vec::new)
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

    #[test]
    fn test_hook_registry_empty() {
        let registry = HookRegistry::new();
        assert!(registry.hooks_by_event.is_empty());
    }

    #[test]
    fn test_hook_registry_missing_dir() {
        let registry = HookRegistry::load_from_directory(&PathBuf::from("/nonexistent")).unwrap();
        assert!(registry.hooks_by_event.is_empty());
    }
}
