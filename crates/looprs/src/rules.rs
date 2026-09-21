use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
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

/// Runtime boundary at which an executable action is requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionBoundary {
    Any,
    Tool,
    McpTool,
    Hook,
    CustomCommand,
    Plugin,
}

/// Result requested by a matching execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    Audit,
    Approval,
    Deny,
}

impl PolicyEffect {
    fn restrictiveness(self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Audit => 1,
            Self::Approval => 2,
            Self::Deny => 3,
        }
    }
}

/// Origin used to make equal-strength policy composition deterministic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicySource {
    User,
    Repository,
    #[default]
    Runtime,
}

/// Typed execution policy loaded from a YAML rule file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub id: String,
    pub effect: PolicyEffect,
    #[serde(default = "any_boundary")]
    pub boundary: ExecutionBoundary,
    #[serde(default = "wildcard_target")]
    pub target: String,
    #[serde(default)]
    pub input_contains: Option<String>,
    pub reason: String,
    #[serde(default)]
    pub audit: BTreeMap<String, String>,
    #[serde(skip, default)]
    pub source: PolicySource,
}

fn any_boundary() -> ExecutionBoundary {
    ExecutionBoundary::Any
}

fn wildcard_target() -> String {
    "*".to_string()
}

/// Canonical request presented to the policy evaluator before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionRequest {
    pub boundary: ExecutionBoundary,
    pub target: String,
    pub input: String,
}

impl ExecutionRequest {
    pub fn new(
        boundary: ExecutionBoundary,
        target: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            boundary,
            target: target.into(),
            input: input.into(),
        }
    }
}

/// Stable, serializable outcome suitable for append-only audit streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyDecision {
    pub effect: PolicyEffect,
    pub reason: String,
    pub matched_policy: Option<String>,
    pub matched_policies: Vec<String>,
    pub audit: BTreeMap<String, String>,
}

/// Closed policy gate failure returned before side effects occur.
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("policy evaluation failed closed: {0}")]
    Evaluation(String),
    #[error("execution denied by policy '{policy}': {reason}")]
    Denied { policy: String, reason: String },
    #[error("execution requires approval by policy '{policy}': {reason}")]
    ApprovalRequired { policy: String, reason: String },
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
    policies: Vec<ExecutionPolicy>,
    policy_error: Option<String>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            policies: Vec::new(),
            policy_error: None,
        }
    }

    /// Register a single rule
    pub fn register(&mut self, rule: Rule) {
        self.rules.insert(rule.id.clone(), rule);
    }

    /// Load rules from a directory (non-recursive)
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<usize, String> {
        self.load_from_directory_with_source(dir, PolicySource::Runtime)
    }

    fn load_from_directory_with_source(
        &mut self,
        dir: &Path,
        source: PolicySource,
    ) -> Result<usize, String> {
        if !dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read rules directory {}: {e}", dir.display()))?;

        let mut loaded = 0;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
            let path = entry.path();

            let extension = path.extension().and_then(|s| s.to_str());
            if path.is_file() && extension == Some("md") {
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
            } else if path.is_file() && matches!(extension, Some("yaml" | "yml")) {
                let content = fs::read_to_string(&path).map_err(|error| {
                    format!("Failed to read policy {}: {error}", path.display())
                })?;
                let mut policies = parse_policy_file(&content).map_err(|error| {
                    format!("Failed to parse policy {}: {error}", path.display())
                })?;
                for policy in &mut policies {
                    policy.source = source;
                    validate_policy(policy)?;
                }
                loaded += policies.len();
                self.policies.extend(policies);
            }
        }

        Ok(loaded)
    }

    /// Load rules from both user and repo directories with repo precedence
    // qual:allow(iosp) reason: "I/O boundary — reads rule files from filesystem"
    pub fn load_all() -> Self {
        let mut registry = Self::new();

        // Load user-level rules first (~/.looprs/rules/)
        if let Ok(home) = std::env::var("HOME") {
            let user_rules_dir = std::path::PathBuf::from(home).join(".looprs").join("rules");
            if let Err(e) =
                registry.load_from_directory_with_source(&user_rules_dir, PolicySource::User)
            {
                eprintln!("⚠️  Failed to load user rules: {e}");
                registry.policy_error = Some(e);
            }
        }

        // Load repo-level rules second (.looprs/rules/) - these override user rules
        let repo_rules_dir = PathBuf::from(".looprs").join("rules");
        if let Err(e) =
            registry.load_from_directory_with_source(&repo_rules_dir, PolicySource::Repository)
        {
            eprintln!("⚠️  Failed to load repo rules: {e}");
            registry.policy_error = Some(e);
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

    /// Get count of loaded rules
    pub fn count(&self) -> usize {
        self.rules.len()
    }

    /// Register an in-memory policy, primarily for embedding and tests.
    pub fn register_policy(&mut self, policy: ExecutionPolicy) {
        self.policies.push(policy);
    }

    /// Evaluate all matching policies using most-restrictive-wins composition.
    pub fn evaluate(&self, request: &ExecutionRequest) -> Result<PolicyDecision, PolicyError> {
        if let Some(error) = &self.policy_error {
            return Err(PolicyError::Evaluation(error.clone()));
        }

        let mut matches = self
            .policies
            .iter()
            .filter(|policy| policy_matches(policy, request))
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            left.effect
                .restrictiveness()
                .cmp(&right.effect.restrictiveness())
                .then(left.source.cmp(&right.source))
                .then(left.id.cmp(&right.id))
        });

        let matched_policies = matches
            .iter()
            .map(|policy| policy.id.clone())
            .collect::<Vec<_>>();
        let Some(selected) = matches.last() else {
            return Ok(PolicyDecision {
                effect: PolicyEffect::Allow,
                reason: "no execution policy matched".to_string(),
                matched_policy: None,
                matched_policies,
                audit: BTreeMap::new(),
            });
        };

        Ok(PolicyDecision {
            effect: selected.effect,
            reason: selected.reason.clone(),
            matched_policy: Some(selected.id.clone()),
            matched_policies,
            audit: selected.audit.clone(),
        })
    }

    /// Authorize a request, failing closed for deny, unapproved, or evaluation failures.
    pub fn authorize(
        &self,
        request: &ExecutionRequest,
        approved: bool,
    ) -> Result<PolicyDecision, PolicyError> {
        let decision = self.evaluate(request)?;
        let policy = decision
            .matched_policy
            .clone()
            .unwrap_or_else(|| "default".to_string());
        match decision.effect {
            PolicyEffect::Deny => Err(PolicyError::Denied {
                policy,
                reason: decision.reason,
            }),
            PolicyEffect::Approval if !approved => Err(PolicyError::ApprovalRequired {
                policy,
                reason: decision.reason,
            }),
            PolicyEffect::Allow | PolicyEffect::Audit | PolicyEffect::Approval => Ok(decision),
        }
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

#[derive(Deserialize)]
#[serde(untagged)]
enum PolicyFile {
    One(ExecutionPolicy),
    Many { policies: Vec<ExecutionPolicy> },
}

fn parse_policy_file(content: &str) -> Result<Vec<ExecutionPolicy>, serde_yaml::Error> {
    match serde_yaml::from_str(content)? {
        PolicyFile::One(policy) => Ok(vec![policy]),
        PolicyFile::Many { policies } => Ok(policies),
    }
}

fn validate_policy(policy: &ExecutionPolicy) -> Result<(), String> {
    if policy.id.trim().is_empty() {
        return Err("Policy id cannot be empty".to_string());
    }
    if policy.target.trim().is_empty() {
        return Err(format!("Policy '{}' target cannot be empty", policy.id));
    }
    if policy.reason.trim().is_empty() {
        return Err(format!("Policy '{}' reason cannot be empty", policy.id));
    }
    Ok(())
}

fn policy_matches(policy: &ExecutionPolicy, request: &ExecutionRequest) -> bool {
    let boundary_matches = policy.boundary == ExecutionBoundary::Any
        || policy.boundary == request.boundary
        || (policy.boundary == ExecutionBoundary::Tool
            && request.boundary == ExecutionBoundary::McpTool);
    boundary_matches
        && wildcard_matches(&policy.target, &request.target)
        && policy.input_contains.as_ref().is_none_or(|needle| {
            normalize_for_policy(&request.input).contains(&normalize_for_policy(needle))
        })
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let value = value.to_ascii_lowercase();
    match (pattern.strip_prefix('*'), pattern.strip_suffix('*')) {
        (Some(suffix), _) if !suffix.ends_with('*') => value.ends_with(suffix),
        (_, Some(prefix)) if !prefix.starts_with('*') => value.starts_with(prefix),
        _ => pattern == "*" || pattern == value,
    }
}

fn normalize_for_policy(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
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

    #[test]
    fn policy_precedence_conflicts_and_audit_are_deterministic() {
        let mut registry = RuleRegistry::new();
        registry.register_policy(ExecutionPolicy {
            id: "user-allow-bash".to_string(),
            effect: PolicyEffect::Allow,
            boundary: ExecutionBoundary::Tool,
            target: "bash".to_string(),
            input_contains: None,
            reason: "user allows bash".to_string(),
            audit: [("ticket".to_string(), "USR-1".to_string())]
                .into_iter()
                .collect(),
            source: PolicySource::User,
        });
        registry.register_policy(ExecutionPolicy {
            id: "repo-deny-shells".to_string(),
            effect: PolicyEffect::Deny,
            boundary: ExecutionBoundary::Tool,
            target: "*ash".to_string(),
            input_contains: None,
            reason: "repository blocks shell tools".to_string(),
            audit: [("control".to_string(), "SEC-7".to_string())]
                .into_iter()
                .collect(),
            source: PolicySource::Repository,
        });

        let request = ExecutionRequest::new(ExecutionBoundary::Tool, "bash", "rm -rf ignored");
        let decision = registry.evaluate(&request).unwrap();

        assert_eq!(decision.effect, PolicyEffect::Deny);
        assert_eq!(decision.matched_policy.as_deref(), Some("repo-deny-shells"));
        assert_eq!(decision.reason, "repository blocks shell tools");
        assert_eq!(
            decision.audit.get("control").map(String::as_str),
            Some("SEC-7")
        );
        assert_eq!(
            serde_json::to_string(&decision).unwrap(),
            serde_json::to_string(&registry.evaluate(&request).unwrap()).unwrap()
        );
    }

    #[test]
    fn approval_and_deny_fail_closed_without_explicit_approval() {
        let mut registry = RuleRegistry::new();
        registry.register_policy(ExecutionPolicy {
            id: "approve-deploy".to_string(),
            effect: PolicyEffect::Approval,
            boundary: ExecutionBoundary::CustomCommand,
            target: "deploy".to_string(),
            input_contains: Some("production".to_string()),
            reason: "production deploys need approval".to_string(),
            audit: Default::default(),
            source: PolicySource::Repository,
        });
        let request = ExecutionRequest::new(
            ExecutionBoundary::CustomCommand,
            "deploy",
            "production --force",
        );

        assert!(registry.authorize(&request, false).is_err());
        assert!(registry.authorize(&request, true).is_ok());
    }

    #[test]
    fn target_and_input_matching_resist_simple_bypass_attempts() {
        let mut registry = RuleRegistry::new();
        registry.register_policy(ExecutionPolicy {
            id: "deny-shell-delete".to_string(),
            effect: PolicyEffect::Deny,
            boundary: ExecutionBoundary::Any,
            target: "*".to_string(),
            input_contains: Some("rm -rf".to_string()),
            reason: "recursive deletion denied".to_string(),
            audit: Default::default(),
            source: PolicySource::User,
        });

        for boundary in [
            ExecutionBoundary::Tool,
            ExecutionBoundary::McpTool,
            ExecutionBoundary::Hook,
            ExecutionBoundary::CustomCommand,
            ExecutionBoundary::Plugin,
        ] {
            let request = ExecutionRequest::new(boundary, "renamed-command", "  RM   -RF /tmp/x ");
            assert_eq!(
                registry.evaluate(&request).unwrap().effect,
                PolicyEffect::Deny
            );
        }
    }
}
