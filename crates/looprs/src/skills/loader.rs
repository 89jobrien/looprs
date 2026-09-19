// Loader for skills from directories

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::SkillRegistry;
use super::discovery::find_skills_in_dir;

impl SkillRegistry {
    /// Load canonical nested `SKILL.md` files and root YAML skill definitions.
    ///
    /// Nested Markdown skills are loaded first, followed by lexically sorted
    /// root-level `.yaml` and `.yml` definitions. A later definition with the
    /// same `name` replaces the earlier one, so root YAML overrides nested
    /// Markdown within one directory. Invalid individual definitions are
    /// reported to stderr and skipped; directory I/O failures are returned.
    // qual:allow(iosp) reason: "I/O boundary — loads skill files from directory"
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize> {
        if !dir.exists() {
            anyhow::bail!("Directory does not exist: {}", dir.display());
        }

        let mut count = 0;

        for discovered in find_skills_in_dir(dir, "internal", 3) {
            match self.load_skill_file(&discovered.skill_file) {
                Ok(_) => count += 1,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load skill from {}: {}",
                        discovered.skill_file.display(),
                        e
                    );
                }
            }
        }

        let mut yaml_files = fs::read_dir(dir)
            .with_context(|| format!("Failed to read skill directory: {}", dir.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && matches!(
                        path.extension().and_then(|extension| extension.to_str()),
                        Some("yaml" | "yml")
                    )
            })
            .collect::<Vec<_>>();
        yaml_files.sort();
        for path in yaml_files {
            match self.load_yaml_skill(&path) {
                Ok(()) => count += 1,
                Err(error) => eprintln!(
                    "Warning: Failed to load skill from {}: {error}",
                    path.display()
                ),
            }
        }

        Ok(count)
    }

    /// Load skills from two directories with repo-over-user precedence.
    ///
    /// Each directory uses [`SkillRegistry::load_from_directory`] validation
    /// and within-directory ordering. The user directory is loaded first and
    /// the repository directory second, so a valid repository skill replaces a
    /// user skill with the same `name`. Missing directories are ignored, while
    /// an existing path that cannot be read as a directory returns an error.
    ///
    /// ```no_run
    /// use std::{fs, path::Path};
    /// use looprs::SkillRegistry;
    ///
    /// let user = Path::new("user-skills");
    /// let repo = Path::new("repo-skills");
    /// fs::create_dir_all(user)?;
    /// fs::create_dir_all(repo)?;
    /// fs::write(
    ///     user.join("review.yaml"),
    ///     "name: review\ntriggers: [review]\ncontent: User policy\n",
    /// )?;
    /// fs::write(
    ///     repo.join("review.yaml"),
    ///     "name: review\ntriggers: [review]\ncontent: Repository policy\n",
    /// )?;
    ///
    /// let mut registry = SkillRegistry::new();
    /// registry.load_with_precedence(user, repo)?;
    /// assert_eq!(registry.get("review").map(|skill| skill.content.as_str()),
    ///            Some("Repository policy"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load_with_precedence(&mut self, user_dir: &Path, repo_dir: &Path) -> Result<usize> {
        // Load user skills first (if directory exists)
        if user_dir.exists() {
            self.load_from_directory(user_dir)?;
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

    fn load_yaml_skill(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read skill file: {}", path.display()))?;
        let skill = super::parser::parse_yaml_skill(path, &content)
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

    #[cfg(unix)]
    struct PermissionGuard {
        path: std::path::PathBuf,
        permissions: fs::Permissions,
    }

    #[cfg(unix)]
    impl Drop for PermissionGuard {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.path, self.permissions.clone());
        }
    }

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
    fn repository_root_yaml_skill_is_discovered() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("repository-skill.yaml"),
            "name: repository-skill\ndescription: Repo skill\ntriggers: [repo]\ncontent: Repository guidance.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        let count = registry.load_from_directory(temp.path()).unwrap();

        assert_eq!(count, 1);
        assert_eq!(
            registry
                .get("repository-skill")
                .map(|skill| skill.content.as_str()),
            Some("Repository guidance.")
        );
    }

    #[test]
    fn repository_root_yml_skill_is_discovered() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("repository-skill.yml"),
            "name: repository-skill\ntriggers: [repo]\ncontent: Repository guidance.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        assert_eq!(registry.load_from_directory(temp.path()).unwrap(), 1);
        assert!(registry.get("repository-skill").is_some());
    }

    #[test]
    fn malformed_and_blank_yaml_are_skipped_without_hiding_valid_skills() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("blank.yml"), "").unwrap();
        fs::write(temp.path().join("malformed.yaml"), "name: [broken").unwrap();
        fs::write(
            temp.path().join("valid.yml"),
            "name: valid\ntriggers: [valid]\ncontent: valid\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        assert_eq!(registry.load_from_directory(temp.path()).unwrap(), 1);
        assert!(registry.get("valid").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_yaml_is_skipped_without_hiding_valid_skills() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let unreadable = temp.path().join("unreadable.yaml");
        fs::write(
            &unreadable,
            "name: unreadable\ntriggers: [hidden]\ncontent: hidden\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("valid.yaml"),
            "name: valid\ntriggers: [valid]\ncontent: visible\n",
        )
        .unwrap();

        let original_permissions = fs::metadata(&unreadable).unwrap().permissions();
        let _guard = PermissionGuard {
            path: unreadable.clone(),
            permissions: original_permissions.clone(),
        };
        let mut denied_permissions = original_permissions;
        denied_permissions.set_mode(0o000);
        fs::set_permissions(&unreadable, denied_permissions).unwrap();

        assert!(
            fs::read_to_string(&unreadable).is_err(),
            "test setup must make the YAML file unreadable"
        );

        let mut registry = SkillRegistry::new();
        assert_eq!(registry.load_from_directory(temp.path()).unwrap(), 1);
        assert!(registry.get("unreadable").is_none());
        assert!(registry.get("valid").is_some());
    }

    #[test]
    fn root_yaml_overrides_nested_markdown_with_same_name() {
        let temp = TempDir::new().unwrap();
        let nested = temp.path().join("shared");
        fs::create_dir(&nested).unwrap();
        fs::write(
            nested.join("SKILL.md"),
            "---\nname: shared\ntriggers: [markdown]\n---\nMarkdown body\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("shared.yml"),
            "name: shared\ntriggers: [yaml]\ncontent: YAML body\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::new();
        assert_eq!(registry.load_from_directory(temp.path()).unwrap(), 2);
        let skill = registry.get("shared").unwrap();
        assert_eq!(skill.triggers, ["yaml"]);
        assert_eq!(skill.content, "YAML body");
    }

    #[test]
    fn unreadable_directory_returns_contextual_error() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("not-a-directory");
        fs::write(&file, "content").unwrap();

        let error = SkillRegistry::new().load_from_directory(&file).unwrap_err();
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn precedence_loader_propagates_existing_user_path_errors() {
        let temp = TempDir::new().unwrap();
        let user_file = temp.path().join("user-file");
        let repo_dir = temp.path().join("repo");
        fs::write(&user_file, "not a directory").unwrap();
        fs::create_dir(&repo_dir).unwrap();

        let error = SkillRegistry::new()
            .load_with_precedence(&user_file, &repo_dir)
            .unwrap_err();

        assert!(error.to_string().contains("skill directory"));
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
