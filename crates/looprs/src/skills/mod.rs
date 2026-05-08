// Skills module - loads SKILL.md files with YAML frontmatter
// Following Anthropic Agent Skills standard

pub mod discovery;
pub mod loader;
pub mod parser;

use std::path::PathBuf;

/// A skill loaded from SKILL.md with YAML frontmatter
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,
    pub content: String,
    pub source_path: PathBuf,
}

/// Registry for loading and matching skills
pub struct SkillRegistry {
    skills: Vec<Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn register(&mut self, skill: Skill) {
        // Remove existing skill with same name (for precedence)
        self.skills.retain(|s| s.name != skill.name);
        self.skills.push(skill);
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn list(&self) -> Vec<&Skill> {
        self.skills.iter().collect()
    }

    /// Find skills with triggers matching the input (case-insensitive substring)
    pub fn find_matching(&self, input: &str) -> Vec<&Skill> {
        let input_lower = input.to_lowercase();
        self.skills
            .iter()
            .filter(|skill| {
                skill
                    .triggers
                    .iter()
                    .any(|trigger| input_lower.contains(&trigger.to_lowercase()))
            })
            .collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_get_by_name() {
        let mut registry = SkillRegistry::new();
        let skill = Skill {
            name: "test-skill".to_string(),
            description: None,
            triggers: vec!["test".to_string()],
            content: "content".to_string(),
            source_path: PathBuf::from("/test"),
        };

        registry.register(skill.clone());

        assert_eq!(registry.get("test-skill"), Some(&skill));
        assert_eq!(registry.get("nonexistent"), None);
    }

    #[test]
    fn test_find_matching_case_insensitive() {
        let mut registry = SkillRegistry::new();
        registry.register(Skill {
            name: "rust-testing".to_string(),
            description: None,
            triggers: vec!["cargo test".to_string(), "rust test".to_string()],
            content: "".to_string(),
            source_path: PathBuf::from("/test"),
        });

        // Case-insensitive matching
        let matches = registry.find_matching("I need help with CARGO TEST");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "rust-testing");
    }

    #[test]
    fn test_find_matching_substring() {
        let mut registry = SkillRegistry::new();
        registry.register(Skill {
            name: "error-handling".to_string(),
            description: None,
            triggers: vec!["? operator".to_string()],
            content: "".to_string(),
            source_path: PathBuf::from("/test"),
        });

        // Substring matching
        let matches = registry.find_matching("How do I use the ? operator in Rust?");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "error-handling");
    }

    #[test]
    fn test_find_matching_multiple_skills() {
        let mut registry = SkillRegistry::new();
        registry.register(Skill {
            name: "skill-1".to_string(),
            description: None,
            triggers: vec!["test".to_string()],
            content: "".to_string(),
            source_path: PathBuf::from("/test1"),
        });
        registry.register(Skill {
            name: "skill-2".to_string(),
            description: None,
            triggers: vec!["testing".to_string()],
            content: "".to_string(),
            source_path: PathBuf::from("/test2"),
        });

        // Both should match "testing" (contains "test")
        let matches = registry.find_matching("testing my code");
        assert_eq!(matches.len(), 2);
    }
}
