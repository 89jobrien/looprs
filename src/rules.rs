use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A constraint or guideline defined in markdown format
#[derive(Debug, Clone)]
pub struct Rule {
    /// Identifier derived from filename (e.g., "security" from "security-rules.md")
    pub id: String,
    /// Rule title extracted from first # heading
    pub title: String,
    /// Full markdown content
    pub content: String,
    /// Category tags (derived from directory structure or frontmatter)
    pub categories: Vec<String>,
    /// Source path for debugging
    pub source: PathBuf,
}

impl Rule {
    /// Parse a rule from markdown file
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read rule file {}: {}", path.display(), e))?;

        // Extract ID from filename (without extension)
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid filename: {}", path.display()))?
            .to_string();

        // Extract title from first # heading
        let title = extract_title(&content).unwrap_or_else(|| id.clone());

        // Extract categories from path components
        let categories = extract_categories(path);

        Ok(Rule {
            id,
            title,
            content,
            categories,
            source: path.to_path_buf(),
        })
    }
}

/// Registry for loading and managing rules
pub struct RuleRegistry {
    rules: HashMap<String, Rule>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    /// Register a single rule
    pub fn register(&mut self, rule: Rule) {
        self.rules.insert(rule.id.clone(), rule);
    }

    /// Load rules from a directory (non-recursive)
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize, String> {
        if !dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read rules directory {}: {e}", dir.display()))?;

        let mut loaded = 0;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
            let path = entry.path();

            // Only process .md files
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                // Skip README files
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("README.md"))
                    .unwrap_or(false)
                {
                    continue;
                }

                match Rule::from_file(&path) {
                    Ok(rule) => {
                        self.register(rule);
                        loaded += 1;
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to load rule from {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Load rules from both user and repo directories with repo precedence
    pub fn load_all() -> Self {
        let mut registry = Self::new();

        // Load user-level rules first (~/.looprs/rules/)
        if let Ok(home) = std::env::var("HOME") {
            let user_rules_dir = std::path::PathBuf::from(home).join(".looprs").join("rules");
            if let Err(e) = registry.load_from_directory(&user_rules_dir) {
                eprintln!("⚠️  Failed to load user rules: {e}");
            }
        }

        // Load repo-level rules second (.looprs/rules/) - these override user rules
        let repo_rules_dir = PathBuf::from(".looprs").join("rules");
        if let Err(e) = registry.load_from_directory(&repo_rules_dir) {
            eprintln!("⚠️  Failed to load repo rules: {e}");
        }

        registry
    }

    /// Get a rule by ID
    pub fn get(&self, id: &str) -> Option<&Rule> {
        self.rules.get(id)
    }

    /// Get all rules
    pub fn all(&self) -> impl Iterator<Item = &Rule> {
        self.rules.values()
    }

    /// Get rules by category
    pub fn get_by_category(&self, category: &str) -> Vec<&Rule> {
        self.rules
            .values()
            .filter(|rule| rule.categories.iter().any(|c| c == category))
            .collect()
    }

    /// Get count of loaded rules
    pub fn count(&self) -> usize {
        self.rules.len()
    }

    /// Format rules for injection into system prompts
    pub fn format_for_prompt(&self) -> String {
        if self.rules.is_empty() {
            return String::new();
        }

        let mut output = String::from("\n## Project Rules and Guidelines\n\n");

        for rule in self.rules.values() {
            output.push_str(&format!("### {}\n\n", rule.title));
            output.push_str(&rule.content);
            output.push_str("\n\n");
        }

        output
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the first # heading from markdown content
fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix('#') {
            let title = heading.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

/// Extract category tags from path structure
/// Example: rules/languages/rust.md -> ["languages", "rust"]
fn extract_categories(path: &Path) -> Vec<String> {
    let mut categories = Vec::new();

    // Get parent directories relative to rules dir
    if let Some(parent) = path.parent() {
        for component in parent.components() {
            if let Some(name) = component.as_os_str().to_str() {
                // Skip "rules" and "." directories
                if name != "rules" && name != "." && name != ".looprs" {
                    categories.push(name.to_string());
                }
            }
        }
    }

    // Add the filename stem as a category
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        categories.push(stem.to_string());
    }

    categories
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_title() {
        let content = "# Security Rules\n\nSome content";
        assert_eq!(extract_title(content), Some("Security Rules".to_string()));

        let no_heading = "Just content";
        assert_eq!(extract_title(no_heading), None);
    }

    #[test]
    fn test_extract_categories() {
        let path = PathBuf::from(".looprs/rules/languages/rust-rules.md");
        let categories = extract_categories(&path);
        assert!(categories.contains(&"languages".to_string()));
        assert!(categories.contains(&"rust-rules".to_string()));
    }

    #[test]
    fn test_rule_from_file() {
        let dir = TempDir::new().unwrap();
        let rule_path = dir.path().join("test-rule.md");

        let mut file = fs::File::create(&rule_path).unwrap();
        writeln!(file, "# Test Rule").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "This is a test rule.").unwrap();

        let rule = Rule::from_file(&rule_path).unwrap();
        assert_eq!(rule.id, "test-rule");
        assert_eq!(rule.title, "Test Rule");
        assert!(rule.content.contains("This is a test rule"));
    }

    #[test]
    fn test_registry_load_and_get() {
        let dir = TempDir::new().unwrap();

        // Create a test rule file
        let rule_path = dir.path().join("security.md");
        let mut file = fs::File::create(&rule_path).unwrap();
        writeln!(file, "# Security Guidelines").unwrap();
        writeln!(file, "Always validate input.").unwrap();

        // Load rules
        let mut registry = RuleRegistry::new();
        let loaded = registry.load_from_directory(dir.path()).unwrap();
        assert_eq!(loaded, 1);

        // Retrieve rule
        let rule = registry.get("security").unwrap();
        assert_eq!(rule.title, "Security Guidelines");
    }

    #[test]
    fn test_registry_skips_readme() {
        let dir = TempDir::new().unwrap();

        // Create README.md
        let readme_path = dir.path().join("README.md");
        fs::File::create(&readme_path).unwrap();

        // Create actual rule
        let rule_path = dir.path().join("real-rule.md");
        let mut file = fs::File::create(&rule_path).unwrap();
        writeln!(file, "# Real Rule").unwrap();

        let mut registry = RuleRegistry::new();
        let loaded = registry.load_from_directory(dir.path()).unwrap();
        assert_eq!(loaded, 1); // Only real-rule.md loaded
        assert!(registry.get("real-rule").is_some());
        assert!(registry.get("README").is_none());
    }
}
