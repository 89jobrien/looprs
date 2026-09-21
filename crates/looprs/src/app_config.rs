//! Defines and loads application configuration from `.looprs/config.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::file_refs::FileRefPolicy;
use crate::fs_mode::FsMode;
use crate::state::AppState;

/// Top-level application configuration, loaded from `.looprs/config.json`
/// via [`AppConfig::load`]. Every field is `#[serde(default)]`, so any
/// subset of keys may be present in the file; missing sections fall back
/// to their type's `Default` impl.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Default provider request settings (context window, temperature,
    /// timeout).
    pub defaults: DefaultsConfig,
    /// Settings controlling `@file` reference resolution.
    pub file_references: FileReferencesConfig,
    /// Tracks whether the first-run demo/onboarding flow has been shown.
    pub onboarding: OnboardingConfig,
    /// Settings for the post-tool-use quality pipeline (checks,
    /// compaction, scoring).
    pub pipeline: PipelineConfig,
    /// Settings for sub-agent selection and delegation.
    pub agents: AgentsConfig,
    /// Directories to load agents, commands, hooks, rules, and skills from.
    pub paths: PathsConfig,
    /// Settings for session persistence.
    pub persistence: PersistenceConfig,
}

impl AppConfig {
    /// Loads configuration from `.looprs/config.json` in the current
    /// working directory, falling back to [`AppConfig::default`] if the
    /// file doesn't exist. `onboarding.demo_seen` is then overlaid from the
    /// app state file (see [`AppState::load`]) so the app never needs to
    /// write back to `config.json` just to persist that flag.
    ///
    /// # Errors
    /// Returns an error if `config.json` exists but cannot be read or
    /// fails to parse as JSON. A missing or invalid state file is ignored
    /// rather than propagated.
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

    /// Builds a [`FileRefPolicy`] from `self.file_references`, for use when
    /// resolving `@file` references in user messages.
    pub fn file_ref_policy(&self) -> FileRefPolicy {
        FileRefPolicy::from_config(&self.file_references)
    }
}

/// Default provider request settings, used unless overridden by
/// [`crate::agent::RuntimeSettings`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    /// Caps the context window, in tokens, used for history compaction and
    /// output-token budgeting. Defaults to `Some(8192)`.
    pub max_context_tokens: Option<u32>,
    /// Sampling temperature passed to the provider. Defaults to
    /// `Some(0.2)`.
    pub temperature: Option<f32>,
    /// Per-inference-request timeout, in seconds. Defaults to
    /// `Some(120)`.
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

/// Settings controlling how `@file` references in user messages are
/// resolved and inlined.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileReferencesConfig {
    /// The marker prefix that triggers file resolution, e.g. `"@"`.
    pub prefix: String,
    /// Maximum file size, in megabytes, that will be inlined.
    pub max_size_mb: u64,
    /// File extensions (without the leading dot) eligible for resolution.
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

/// Tracks first-run onboarding state persisted via the app state file (not
/// `config.json` itself; see [`AppConfig::load`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OnboardingConfig {
    /// Whether the onboarding demo has already been shown to the user.
    pub demo_seen: bool,
}

/// Settings for the post-tool-use quality pipeline run by
/// [`crate::agent::Agent::run_turn`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineConfig {
    /// Whether pipeline checks run at all. Disabled by default.
    pub enabled: bool,
    /// Directory pipeline/agent logs are written to.
    pub log_dir: String,
    /// Minimum reward score below which a run is considered a failure.
    pub reward_threshold: f32,
    /// Whether at least one tool call is required for a turn to pass.
    pub require_tools: bool,
    /// If a check fails, whether to roll the in-memory conversation back
    /// to the pre-tool-call snapshot.
    pub auto_revert: bool,
    /// Whether to stop at the first failing check instead of running the
    /// rest.
    pub fail_fast: bool,
    /// Whether a failing check blocks the turn (returns an error) rather
    /// than just being reported.
    pub block_on_failure: bool,
    /// Which individual checks (build/tests/lint/etc.) are enabled.
    pub checks: PipelineChecksConfig,
    /// Settings for the repo-context block compacted into the system
    /// prompt.
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

/// Settings for sub-agent selection and delegation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Whether delegated sub-agents share the parent's conversation
    /// context.
    pub context_sharing: bool,
    /// Maximum number of sub-agents that may run concurrently. Not yet
    /// enforced: [`crate::agent::Agent::run_turn`] always orchestrates
    /// sequentially regardless of this value.
    pub max_parallel: usize,
    /// The orchestration strategy name; only `"sequential"` currently has
    /// an effect.
    pub orchestration: String,
    /// Whether to delegate to the first registered agent when no agent's
    /// triggers match the prompt and no `default_agent` is set.
    pub delegate_by_default: bool,
    /// Filesystem access mode applied to delegated sub-agents.
    pub fs_mode: FsMode,
    /// Name of the agent to delegate to when no trigger matches the
    /// prompt.
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

/// Toggles for individual pipeline checks; all default to `false`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PipelineChecksConfig {
    /// Run the project build as a check.
    pub run_build: bool,
    /// Run the test suite as a check.
    pub run_tests: bool,
    /// Run the linter as a check.
    pub run_lint: bool,
    /// Run type-checking as a check.
    pub run_typecheck: bool,
    /// Run benchmarks as a check.
    pub run_bench: bool,
}

/// Settings for the repo-context block injected into the system prompt
/// (see [`crate::pipeline::context_compact::compact_context`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineCompactionConfig {
    /// Include the current working-tree diff in the injected context.
    pub include_diff: bool,
    /// Include a list of recently modified files.
    pub include_recent: bool,
    /// Extra glob patterns whose matching files are also included.
    pub include_globs: Vec<String>,
    /// Maximum number of items included per category.
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

/// Backend used to persist session logs (see
/// [`crate::ports::SessionStore`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStoreBackend {
    /// Filesystem JSONL per session (default, no setup required).
    #[default]
    Fs,
    /// SQLite database at `~/.looprs/sessions.db`.
    Sqlite,
}

/// Settings for session persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersistenceConfig {
    /// Which session store backend to use.
    pub session_store: SessionStoreBackend,
}

/// Directories `looprs` loads its extensibility artifacts from, relative
/// to the working directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    /// Directory containing agent definition YAML files.
    pub agents: String,
    /// Directory containing custom slash command definitions.
    pub commands: String,
    /// Directory containing lifecycle hook YAML files.
    pub hooks: String,
    /// Directory containing rule files injected into the system prompt.
    pub rules: String,
    /// Directory containing skill definitions.
    pub skills: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            agents: ".looprs/agents".to_string(),
            commands: ".looprs/commands".to_string(),
            hooks: ".looprs/hooks".to_string(),
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
    }
}
