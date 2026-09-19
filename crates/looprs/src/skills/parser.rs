// Parser for SKILL.md files with YAML frontmatter

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct SkillDefinition {
    name: String,
    #[serde(default)]
    description: Option<String>,
    triggers: Vec<String>,
    #[serde(default)]
    content: String,
}

impl SkillDefinition {
    fn into_skill(mut self, path: &Path, content: Option<&str>) -> Result<super::Skill> {
        self.name = self.name.trim().to_string();
        self.triggers = self
            .triggers
            .into_iter()
            .map(|trigger| trigger.trim().to_string())
            .collect();
        self.content = content.unwrap_or(&self.content).trim().to_string();

        if self.name.is_empty() {
            anyhow::bail!("Skill name cannot be empty");
        }
        if self.triggers.is_empty() || self.triggers.iter().any(String::is_empty) {
            anyhow::bail!(
                "Skill must have at least one trigger and all triggers must be non-empty"
            );
        }
        if self.content.is_empty() {
            anyhow::bail!("Skill content cannot be empty");
        }

        Ok(super::Skill {
            name: self.name,
            description: self
                .description
                .map(|description| description.trim().to_string())
                .filter(|description| !description.is_empty()),
            triggers: self.triggers,
            content: self.content,
            source_path: path.to_path_buf(),
        })
    }
}

/// Parse SKILL.md file with YAML frontmatter  
pub fn parse_skill_file(path: &Path, content: &str) -> Result<super::Skill> {
    let after_opening = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
        .context("Invalid SKILL.md format: missing YAML frontmatter delimiters")?;
    let mut offset = 0;
    let (frontmatter_text, body) = after_opening
        .split_inclusive('\n')
        .find_map(|line| {
            let line_start = offset;
            offset += line.len();
            (line.trim_end_matches(['\r', '\n']) == "---")
                .then(|| (&after_opening[..line_start], &after_opening[offset..]))
        })
        .context("Invalid SKILL.md format: missing YAML frontmatter delimiters")?;

    // Parse YAML frontmatter
    let frontmatter: SkillDefinition =
        serde_yaml::from_str(frontmatter_text).context("Failed to parse YAML frontmatter")?;
    frontmatter.into_skill(path, Some(body))
}

/// Parse a repository YAML skill definition.
pub fn parse_yaml_skill(path: &Path, content: &str) -> Result<super::Skill> {
    let skill: SkillDefinition =
        serde_yaml::from_str(content).context("Failed to parse YAML skill")?;
    skill.into_skill(path, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    // ── Property tests ──────────────────────────────────────────────────

    proptest! {
        #[test]
        fn parse_never_panics(content in "\\PC{0,500}") {
            let path = PathBuf::from("/test/SKILL.md");
            let _ = parse_skill_file(&path, &content);
        }

        #[test]
        fn valid_frontmatter_round_trips(
            name in "[a-z][a-z0-9]{1,20}",
            trigger in "[a-z]{1,30}",
            body in "[a-zA-Z0-9][a-zA-Z0-9 ]{0,99}",
        ) {
            // Use serde_yaml to safely encode the trigger value
            let trigger_yaml = serde_yaml::to_string(&trigger).unwrap();
            let content = format!(
                "---\nname: {name}\ntriggers:\n  - {trigger_yaml}---\n{body}"
            );
            let path = PathBuf::from("/test/SKILL.md");
            let skill = parse_skill_file(&path, &content)
                .expect("valid frontmatter should parse");
            prop_assert_eq!(&skill.name, &name);
            prop_assert_eq!(&skill.triggers[0], &trigger);
            prop_assert_eq!(&skill.source_path, &path);
        }

        #[test]
        fn parsed_name_is_never_empty(
            name in "[a-z][a-z0-9]{1,20}",
            trigger in "[a-z]{1,10}",
        ) {
            let trigger_yaml = serde_yaml::to_string(&trigger).unwrap();
            let content = format!(
                "---\nname: {name}\ntriggers:\n  - {trigger_yaml}---\ncontent"
            );
            let path = PathBuf::from("/test/SKILL.md");
            let skill = parse_skill_file(&path, &content).unwrap();
            prop_assert!(!skill.name.is_empty());
            prop_assert!(!skill.triggers.is_empty());
        }

        #[test]
        fn yaml_skill_round_trips_valid_fields(
            name in "[a-z][a-z0-9-]{1,20}",
            trigger in "[a-z]{1,30}",
            body in "[a-zA-Z0-9][a-zA-Z0-9 ]{0,99}",
        ) {
            let yaml = serde_yaml::to_string(&serde_json::json!({
                "name": name,
                "triggers": [trigger],
                "content": body,
            })).expect("generated skill must serialize");
            let path = PathBuf::from("/test/skill.yml");
            let skill = parse_yaml_skill(&path, &yaml).expect("valid YAML skill must parse");
            prop_assert!(!skill.name.trim().is_empty());
            prop_assert!(skill.triggers.iter().all(|value| !value.trim().is_empty()));
            prop_assert!(!skill.content.trim().is_empty());
        }
    }

    // ── Unit tests ──────────────────────────────────────────────────────

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
        assert_eq!(
            skill.description,
            Some("Guide for writing Rust tests".to_string())
        );
        assert_eq!(skill.triggers, vec!["rust test", "cargo test"]);
        assert_eq!(
            skill.content,
            "# Rust Testing\n\nThis is the skill content."
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing YAML frontmatter")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("name cannot be empty")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one trigger")
        );
    }

    #[test]
    fn yaml_and_markdown_skills_share_validation() {
        let markdown = "---\nname: '   '\ntriggers: [valid]\n---\ncontent";
        let yaml = "name: valid\ntriggers: ['   ']\ncontent: content\n";

        assert!(parse_skill_file(Path::new("SKILL.md"), markdown).is_err());
        assert!(parse_yaml_skill(Path::new("skill.yaml"), yaml).is_err());
    }

    #[test]
    fn markdown_delimiters_are_recognized_only_on_their_own_lines() {
        let content = "---\nname: a---b\ntriggers: [go]\n---\nbody --- text\n";

        let skill = parse_skill_file(Path::new("SKILL.md"), content).unwrap();

        assert_eq!(skill.name, "a---b");
        assert_eq!(skill.content, "body --- text");
    }

    #[test]
    fn yaml_parser_rejects_blank_malformed_and_blank_content() {
        for content in [
            "",
            "name: [broken",
            "name: valid\ntriggers: [go]\ncontent: '   '\n",
        ] {
            assert!(parse_yaml_skill(Path::new("skill.yml"), content).is_err());
        }
    }
}
