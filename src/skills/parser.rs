// Parser for SKILL.md files with YAML frontmatter

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: Option<String>,
    triggers: Vec<String>,
}

/// Parse SKILL.md file with YAML frontmatter  
pub fn parse_skill_file(path: &Path, content: &str) -> Result<super::Skill> {
    // Split frontmatter from content
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    
    if parts.len() < 3 {
        anyhow::bail!("Invalid SKILL.md format: missing YAML frontmatter delimiters");
    }

    // Parse YAML frontmatter
    let frontmatter: SkillFrontmatter = serde_yaml::from_str(parts[1])
        .context("Failed to parse YAML frontmatter")?;

    // Validate required fields
    if frontmatter.name.is_empty() {
        anyhow::bail!("Skill name cannot be empty");
    }
    if frontmatter.triggers.is_empty() {
        anyhow::bail!("Skill must have at least one trigger");
    }

    Ok(super::Skill {
        name: frontmatter.name,
        description: frontmatter.description,
        triggers: frontmatter.triggers,
        content: parts[2].trim().to_string(),
        source_path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_valid_skill_with_description() {
        let content = r#"---
name: rust-testing
description: Guide for writing Rust tests
triggers:
  - "rust test"
  - "cargo test"
---

# Rust Testing

This is the skill content.
"#;

        let path = PathBuf::from("/test/rust-testing/SKILL.md");
        let skill = parse_skill_file(&path, content).unwrap();

        assert_eq!(skill.name, "rust-testing");
        assert_eq!(skill.description, Some("Guide for writing Rust tests".to_string()));
        assert_eq!(skill.triggers, vec!["rust test", "cargo test"]);
        assert_eq!(skill.content, "# Rust Testing\n\nThis is the skill content.");
        assert_eq!(skill.source_path, path);
    }

    #[test]
    fn test_parse_skill_without_description() {
        let content = r#"---
name: minimal-skill
triggers:
  - "test"
---

# Minimal Skill

Content only.
"#;

        let path = PathBuf::from("/test/minimal/SKILL.md");
        let skill = parse_skill_file(&path, content).unwrap();

        assert_eq!(skill.name, "minimal-skill");
        assert_eq!(skill.description, None);
        assert_eq!(skill.triggers, vec!["test"]);
        assert_eq!(skill.content, "# Minimal Skill\n\nContent only.");
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let content = "# Just Content\n\nNo frontmatter.";
        let path = PathBuf::from("/test/SKILL.md");
        
        let result = parse_skill_file(&path, content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing YAML frontmatter"));
    }

    #[test]
    fn test_parse_empty_name() {
        let content = r#"---
name: ""
triggers:
  - "test"
---
Content
"#;
        let path = PathBuf::from("/test/SKILL.md");
        
        let result = parse_skill_file(&path, content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name cannot be empty"));
    }

    #[test]
    fn test_parse_no_triggers() {
        let content = r#"---
name: test-skill
triggers: []
---
Content
"#;
        let path = PathBuf::from("/test/SKILL.md");
        
        let result = parse_skill_file(&path, content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one trigger"));
    }
}
