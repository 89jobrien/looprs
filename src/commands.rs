use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A custom command definition loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub action: CommandAction,
}

/// Action to execute when command is invoked
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CommandAction {
    #[serde(rename = "prompt")]
    Prompt {
        template: String,
        #[serde(default)]
        variables: HashMap<String, String>,
    },
    #[serde(rename = "shell")]
    Shell {
        command: String,
        #[serde(default)]
        inject_output: bool,
    },
    #[serde(rename = "message")]
    Message { text: String },
}

/// Registry of custom commands
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        CommandRegistry {
            commands: HashMap::new(),
        }
    }

    /// Load commands from a directory
    pub fn load_from_directory(dir: &PathBuf) -> anyhow::Result<Self> {
        let mut registry = CommandRegistry::new();

        if !dir.exists() {
            return Ok(registry);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match Self::parse_command(&path) {
                    Ok(command) => {
                        registry.register(command);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load command {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(registry)
    }

    /// Parse a command from YAML file
    fn parse_command(path: &std::path::Path) -> anyhow::Result<Command> {
        let contents = fs::read_to_string(path)?;
        let command: Command = serde_yaml::from_str(&contents)?;
        Ok(command)
    }

    /// Register a command
    pub fn register(&mut self, command: Command) {
        // Register by name
        self.commands.insert(command.name.clone(), command.clone());
        
        // Register by aliases
        for alias in &command.aliases {
            self.commands.insert(alias.clone(), command.clone());
        }
    }

    /// Get a command by name or alias
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    /// List all registered commands (deduplicated by name)
    pub fn list(&self) -> Vec<&Command> {
        let mut seen = std::collections::HashSet::new();
        let mut commands = Vec::new();
        
        for cmd in self.commands.values() {
            if seen.insert(&cmd.name) {
                commands.push(cmd);
            }
        }
        
        commands.sort_by_key(|c| &c.name);
        commands
    }

    /// Check if a command exists
    pub fn has(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_command_file(dir: &std::path::Path, filename: &str, content: &str) {
        let path = dir.join(filename);
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn test_empty_registry() {
        let registry = CommandRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_load_from_missing_directory() {
        let registry = CommandRegistry::load_from_directory(&PathBuf::from("/nonexistent")).unwrap();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_load_prompt_command() {
        let temp_dir = TempDir::new().unwrap();
        create_test_command_file(
            temp_dir.path(),
            "refactor.yaml",
            r#"name: refactor
description: Refactor code for readability
action:
  type: prompt
  template: "Refactor this code: {code}"
"#,
        );

        let registry = CommandRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(registry.list().len(), 1);

        let cmd = registry.get("refactor").unwrap();
        assert_eq!(cmd.name, "refactor");
        assert_eq!(cmd.description, "Refactor code for readability");
        
        match &cmd.action {
            CommandAction::Prompt { template, .. } => {
                assert_eq!(template, "Refactor this code: {code}");
            }
            _ => panic!("Expected Prompt action"),
        }
    }

    #[test]
    fn test_load_shell_command() {
        let temp_dir = TempDir::new().unwrap();
        create_test_command_file(
            temp_dir.path(),
            "lint.yaml",
            r#"name: lint
description: Run linter
action:
  type: shell
  command: cargo clippy
  inject_output: true
"#,
        );

        let registry = CommandRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        let cmd = registry.get("lint").unwrap();
        
        match &cmd.action {
            CommandAction::Shell { command, inject_output } => {
                assert_eq!(command, "cargo clippy");
                assert!(inject_output);
            }
            _ => panic!("Expected Shell action"),
        }
    }

    #[test]
    fn test_command_with_aliases() {
        let temp_dir = TempDir::new().unwrap();
        create_test_command_file(
            temp_dir.path(),
            "test.yaml",
            r#"name: test
description: Run tests
aliases:
  - t
  - tests
action:
  type: shell
  command: cargo test
"#,
        );

        let registry = CommandRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        
        assert!(registry.has("test"));
        assert!(registry.has("t"));
        assert!(registry.has("tests"));
        
        let cmd1 = registry.get("test").unwrap();
        let cmd2 = registry.get("t").unwrap();
        assert_eq!(cmd1.name, cmd2.name);
    }

    #[test]
    fn test_list_deduplicates() {
        let temp_dir = TempDir::new().unwrap();
        create_test_command_file(
            temp_dir.path(),
            "cmd.yaml",
            r#"name: example
description: Test
aliases:
  - ex
  - e
action:
  type: message
  text: "Hello"
"#,
        );

        let registry = CommandRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        
        // Should only list once despite 3 entries (name + 2 aliases)
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn test_message_action() {
        let temp_dir = TempDir::new().unwrap();
        create_test_command_file(
            temp_dir.path(),
            "help.yaml",
            r#"name: help
description: Show help
action:
  type: message
  text: "Available commands: /refactor, /lint, /test"
"#,
        );

        let registry = CommandRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();
        let cmd = registry.get("help").unwrap();
        
        match &cmd.action {
            CommandAction::Message { text } => {
                assert!(text.contains("Available commands"));
            }
            _ => panic!("Expected Message action"),
        }
    }
}
