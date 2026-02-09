// Loader for skills from directories

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::SkillRegistry;

impl SkillRegistry {
    /// Load skills from a directory (recursively finds SKILL.md files)
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize> {
        if !dir.exists() {
            anyhow::bail!("Directory does not exist: {}", dir.display());
        }

        let mut count = 0;

        // Walk directory recursively looking for SKILL.md files
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Look for SKILL.md files
            if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                // Try to parse and register
                match self.load_skill_file(path) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        // Log error but continue loading other skills
                        eprintln!(
                            "Warning: Failed to load skill from {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(count)
    }

    /// Load skills from two directories with precedence (repo overrides user)
    pub fn load_with_precedence(&mut self, user_dir: &Path, repo_dir: &Path) -> Result<usize> {
        // Load user skills first (if directory exists)
        if user_dir.exists() {
            let _ = self.load_from_directory(user_dir);
        }

        // Load repo skills - these will override user skills with same name
        if repo_dir.exists() {
            let _ = self.load_from_directory(repo_dir)?;
        }

        // Return total count
        Ok(self.skills.len())
    }

    fn load_skill_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read skill file: {}", path.display()))?;

        let skill = super::parser::parse_skill_file(path, &content)
            .with_context(|| format!("Failed to parse skill file: {}", path.display()))?;

        self.register(skill);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_from_empty_directory() {
        let temp = TempDir::new().unwrap();
        let mut registry = SkillRegistry::new();

        let count = registry.load_from_directory(temp.path()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_load_single_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");
        fs::create_dir(&skill_dir).unwrap();

        let skill_content = r#"---
name: test-skill
triggers:
  - "test"
---

# Test Skill

Content here.
"#;
        fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry.load_from_directory(temp.path()).unwrap();

        assert_eq!(count, 1);
        let skill = registry.get("test-skill").unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.triggers, vec!["test"]);
    }

    #[test]
    fn test_load_multiple_skills_nested() {
        let temp = TempDir::new().unwrap();

        // Create nested directory structure: rust/testing/SKILL.md
        let rust_testing = temp.path().join("rust/testing");
        fs::create_dir_all(&rust_testing).unwrap();
        fs::write(
            rust_testing.join("SKILL.md"),
            r#"---
name: rust-testing
triggers:
  - "cargo test"
---
Testing guide.
"#,
        )
        .unwrap();

        // Create rust/error-handling/SKILL.md
        let rust_errors = temp.path().join("rust/error-handling");
        fs::create_dir_all(&rust_errors).unwrap();
        fs::write(
            rust_errors.join("SKILL.md"),
            r#"---
name: rust-error-handling
triggers:
  - "Result type"
---
Error handling guide.
"#,
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry.load_from_directory(temp.path()).unwrap();

        assert_eq!(count, 2);
        assert!(registry.get("rust-testing").is_some());
        assert!(registry.get("rust-error-handling").is_some());
    }

    #[test]
    fn test_skip_invalid_skill_files() {
        let temp = TempDir::new().unwrap();

        // Valid skill
        let valid_dir = temp.path().join("valid");
        fs::create_dir(&valid_dir).unwrap();
        fs::write(
            valid_dir.join("SKILL.md"),
            r#"---
name: valid
triggers:
  - "test"
---
Content.
"#,
        )
        .unwrap();

        // Invalid skill (no frontmatter)
        let invalid_dir = temp.path().join("invalid");
        fs::create_dir(&invalid_dir).unwrap();
        fs::write(invalid_dir.join("SKILL.md"), "Just content, no frontmatter").unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry.load_from_directory(temp.path()).unwrap();

        // Should load 1 valid skill, skip invalid
        assert_eq!(count, 1);
        assert!(registry.get("valid").is_some());
    }

    #[test]
    fn test_nonexistent_directory() {
        let mut registry = SkillRegistry::new();
        let result = registry.load_from_directory(Path::new("/nonexistent/path"));

        assert!(result.is_err());
    }

    #[test]
    fn test_load_with_precedence_repo_overrides_user() {
        let user_temp = TempDir::new().unwrap();
        let repo_temp = TempDir::new().unwrap();

        // User skill
        let user_skill = user_temp.path().join("shared-skill");
        fs::create_dir(&user_skill).unwrap();
        fs::write(
            user_skill.join("SKILL.md"),
            r#"---
name: shared-skill
triggers:
  - "user trigger"
---
User version content.
"#,
        )
        .unwrap();

        // Repo skill (same name, should override)
        let repo_skill = repo_temp.path().join("shared-skill");
        fs::create_dir(&repo_skill).unwrap();
        fs::write(
            repo_skill.join("SKILL.md"),
            r#"---
name: shared-skill
triggers:
  - "repo trigger"
---
Repo version content.
"#,
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry
            .load_with_precedence(user_temp.path(), repo_temp.path())
            .unwrap();

        // Should load 1 skill (repo overrides user)
        assert_eq!(count, 1);

        let skill = registry.get("shared-skill").unwrap();
        assert_eq!(skill.triggers, vec!["repo trigger"]);
        assert!(skill.content.contains("Repo version"));
    }

    #[test]
    fn test_load_with_precedence_combines_unique_skills() {
        let user_temp = TempDir::new().unwrap();
        let repo_temp = TempDir::new().unwrap();

        // User-only skill
        let user_skill = user_temp.path().join("user-skill");
        fs::create_dir(&user_skill).unwrap();
        fs::write(
            user_skill.join("SKILL.md"),
            r#"---
name: user-skill
triggers:
  - "user"
---
User skill.
"#,
        )
        .unwrap();

        // Repo-only skill
        let repo_skill = repo_temp.path().join("repo-skill");
        fs::create_dir(&repo_skill).unwrap();
        fs::write(
            repo_skill.join("SKILL.md"),
            r#"---
name: repo-skill
triggers:
  - "repo"
---
Repo skill.
"#,
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry
            .load_with_precedence(user_temp.path(), repo_temp.path())
            .unwrap();

        // Should load both skills
        assert_eq!(count, 2);
        assert!(registry.get("user-skill").is_some());
        assert!(registry.get("repo-skill").is_some());
    }

    #[test]
    fn test_load_with_precedence_missing_user_dir() {
        let repo_temp = TempDir::new().unwrap();

        let repo_skill = repo_temp.path().join("skill");
        fs::create_dir(&repo_skill).unwrap();
        fs::write(
            repo_skill.join("SKILL.md"),
            r#"---
name: skill
triggers:
  - "test"
---
Content.
"#,
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry
            .load_with_precedence(Path::new("/nonexistent/user"), repo_temp.path())
            .unwrap();

        // Should still load repo skills even if user dir doesn't exist
        assert_eq!(count, 1);
        assert!(registry.get("skill").is_some());
    }
}
