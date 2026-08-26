use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::file_refs::FileRefPolicy;
use crate::fs_mode::FsMode;
use crate::state::AppState;

/// Top-level `.looprs/config.json` schema.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Global defaults used by the runtime.
    pub defaults: DefaultsConfig,
    /// File-reference parsing and safety limits.
    pub file_references: FileReferencesConfig,
    /// One-time onboarding flags.
    pub onboarding: OnboardingConfig,
    /// Optional pipeline execution settings.
    pub pipeline: PipelineConfig,
    /// Multi-agent delegation settings.
    pub agents: AgentsConfig,
    /// Filesystem locations for extensibility assets.
    pub paths: PathsConfig,
    /// Persistence backend selection.
    pub persistence: PersistenceConfig,
}

impl AppConfig {
    /// Load from user-owned `.looprs/config.json`, then overlay onboarding from app state file.
    // qual:allow(iosp) reason: "I/O boundary — reads config file and deserializes"
    pub fn load() -> anyhow::Result<Self> {
        let path = Path::new(".looprs/config.json");
        let mut config: Self = if path.exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content)?
        } else {
            Self::default()
        };
        // State file (e.g. onboarding.demo_seen) overrides so app never writes config.json
        if let Ok(state) = AppState::load() {
            config.onboarding.demo_seen = state.onboarding.demo_seen;
        }
        Ok(config)
    }

    /// Build the effective file-reference policy from config.
    pub fn file_ref_policy(&self) -> FileRefPolicy {
        FileRefPolicy::from_config(&self.file_references)
    }
}

/// Defaults for model/runtime behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    /// Soft context cap used by runtime heuristics.
    pub max_context_tokens: Option<u32>,
    /// Default sampling temperature.
    pub temperature: Option<f32>,
    /// Default request timeout in seconds.
    pub timeout_seconds: Option<u64>,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: Some(8192),
            temperature: Some(0.2),
            timeout_seconds: Some(120),
        }
    }
}

/// Settings for `@file` references in user prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileReferencesConfig {
    /// Prefix used to detect file references.
    pub prefix: String,
    /// Maximum allowed referenced file size (MB).
    pub max_size_mb: u64,
    /// File extensions permitted for inclusion.
    pub allowed_extensions: Vec<String>,
}

impl Default for FileReferencesConfig {
    fn default() -> Self {
        Self {
            prefix: "@".to_string(),
            max_size_mb: 10,
            allowed_extensions: vec![
                "rs", "py", "ts", "js", "go", "java", "md", "txt", "json", "yaml", "toml",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Onboarding state persisted outside `config.json` when possible.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OnboardingConfig {
    /// Whether the onboarding demo has already been shown.
    pub demo_seen: bool,
}

/// Deterministic pipeline runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineConfig {
    /// Enable pipeline execution.
    pub enabled: bool,
    /// Directory where JSONL pipeline logs are written.
    pub log_dir: String,
    /// Minimum score required for pipeline success.
    pub reward_threshold: f32,
    /// Require external tools to be present.
    pub require_tools: bool,
    /// Revert worktree changes on pipeline failure.
    pub auto_revert: bool,
    /// Stop pipeline at first failing step.
    pub fail_fast: bool,
    /// Block normal turn completion if pipeline fails.
    pub block_on_failure: bool,
    /// Build/test/lint gate toggles.
    pub checks: PipelineChecksConfig,
    /// Context compaction inputs and limits.
    pub compaction: PipelineCompactionConfig,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_dir: ".looprs/agent_logs/".to_string(),
            reward_threshold: 0.0,
            require_tools: false,
            auto_revert: true,
            fail_fast: false,
            block_on_failure: false,
            checks: PipelineChecksConfig::default(),
            compaction: PipelineCompactionConfig::default(),
        }
    }
}

/// Delegation and orchestration defaults for multi-agent flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Share contextual data between delegated agents.
    pub context_sharing: bool,
    /// Maximum number of concurrent delegated agents.
    pub max_parallel: usize,
    /// Orchestration strategy label.
    pub orchestration: String,
    /// Delegate automatically when no explicit agent is requested.
    pub delegate_by_default: bool,
    /// Default filesystem mode for delegated execution.
    pub fs_mode: FsMode,
    /// Optional preferred agent name.
    pub default_agent: Option<String>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            context_sharing: true,
            max_parallel: 3,
            orchestration: "sequential".to_string(),
            delegate_by_default: true,
            fs_mode: FsMode::Write,
            default_agent: None,
        }
    }
}

/// On/off switches for pipeline checks.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PipelineChecksConfig {
    /// Run build check.
    pub run_build: bool,
    /// Run tests.
    pub run_tests: bool,
    /// Run linting.
    pub run_lint: bool,
    /// Run type-check gate.
    pub run_typecheck: bool,
    /// Run benchmark gate.
    pub run_bench: bool,
}

/// Inputs and limits for pipeline context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineCompactionConfig {
    /// Include git diff snippets.
    pub include_diff: bool,
    /// Include recently changed files.
    pub include_recent: bool,
    /// Additional include globs.
    pub include_globs: Vec<String>,
    /// Maximum number of relevance-ranked files to include.
    pub top_k: usize,
}

impl Default for PipelineCompactionConfig {
    fn default() -> Self {
        Self {
            include_diff: true,
            include_recent: true,
            include_globs: Vec::new(),
            top_k: 8,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStoreBackend {
    /// Filesystem JSONL per session (default, no setup required).
    #[default]
    Fs,
    /// SQLite database at `~/.looprs/sessions.db`.
    Sqlite,
}

/// Persistence backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersistenceConfig {
    /// Which session store backend to use.
    pub session_store: SessionStoreBackend,
}

/// Filesystem locations for user/repo extension assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    /// Directory containing agent definition files.
    pub agents: String,
    /// Directory containing slash command files.
    pub commands: String,
    /// Directory containing hook definition files.
    pub hooks: String,
    /// Directory containing plugin files.
    pub plugins: String,
    /// Directory containing rule definition files.
    pub rules: String,
    /// Directory containing skill definition files.
    pub skills: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            agents: ".looprs/agents".to_string(),
            commands: ".looprs/commands".to_string(),
            hooks: ".looprs/hooks".to_string(),
            plugins: ".looprs/plugins".to_string(),
            rules: ".looprs/rules".to_string(),
            skills: ".looprs/skills".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn onboarding_demo_seen_defaults_false() {
        let cfg = AppConfig::default();
        assert!(!cfg.onboarding.demo_seen);
    }

    #[test]
    fn load_overlays_onboarding_from_state_file() {
        let tmp = TempDir::new().unwrap();
        let looprs = tmp.path().join(".looprs");
        std::fs::create_dir_all(&looprs).unwrap();
        std::fs::write(
            looprs.join("config.json"),
            r#"{ "onboarding": { "demo_seen": false } }"#,
        )
        .unwrap();
        std::fs::write(
            looprs.join("state.json"),
            r#"{ "onboarding": { "demo_seen": true } }"#,
        )
        .unwrap();
        let original = env::current_dir().unwrap();
        let _ = env::set_current_dir(tmp.path());
        let cfg = AppConfig::load().unwrap();
        let _ = env::set_current_dir(original);
        assert!(
            cfg.onboarding.demo_seen,
            "state file should override config"
        );
    }

    #[test]
    fn test_pipeline_config_defaults_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert!(!decoded.pipeline.enabled);
        assert_eq!(decoded.pipeline.log_dir, ".looprs/agent_logs/");
        assert_eq!(decoded.paths.plugins, ".looprs/plugins");
    }
}
