//! App-managed state (e.g. onboarding flags). Stored in `.looprs/state.json`.
//! User-owned config lives in `.looprs/config.json` and is never written by the app.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DEFAULT_STATE_PATH: &str = ".looprs/state.json";

/// App-managed session/onboarding state, persisted to `.looprs/state.json`
/// (as opposed to user-owned `.looprs/config.json`, which the app never
/// writes to).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    /// Onboarding-flow state.
    pub onboarding: OnboardingState,
}

/// Tracks whether the first-run demo has been shown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OnboardingState {
    /// Whether the onboarding demo has already been shown to the user.
    pub demo_seen: bool,
}

impl AppState {
    /// Loads state from `.looprs/state.json` in the current working
    /// directory.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed as
    /// JSON.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_at(Path::new(DEFAULT_STATE_PATH))
    }

    /// Loads state from `path`, returning [`AppState::default`] if the
    /// file doesn't exist.
    ///
    /// # Errors
    /// Returns an error if `path` exists but cannot be read or parsed as
    /// JSON.
    // qual:allow(iosp) reason: "I/O boundary — reads state file and deserializes"
    pub fn load_at(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Set onboarding.demo_seen and persist to state file only (never touches config.json).
    pub fn set_onboarding_demo_seen(value: bool) -> anyhow::Result<()> {
        Self::set_onboarding_demo_seen_at(Path::new(DEFAULT_STATE_PATH), value)
    }

    /// Loads the state at `path`, sets `onboarding.demo_seen` to `value`,
    /// and writes the result back to `path` as pretty-printed JSON.
    /// Creates `path`'s parent directories if needed.
    ///
    /// # Errors
    /// Returns an error if the existing state cannot be loaded, the parent
    /// directory cannot be created, or the file cannot be written.
    pub fn set_onboarding_demo_seen_at(path: &Path, value: bool) -> anyhow::Result<()> {
        let mut state = Self::load_at(path)?;
        state.onboarding.demo_seen = value;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let content = serde_json::to_string_pretty(&state)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");
        let state = AppState::load_at(&path).unwrap();
        assert!(!state.onboarding.demo_seen);
    }

    #[test]
    fn set_onboarding_demo_seen_persists() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");
        AppState::set_onboarding_demo_seen_at(&path, true).unwrap();
        let state = AppState::load_at(&path).unwrap();
        assert!(state.onboarding.demo_seen);
    }

    #[test]
    fn set_creates_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested/.looprs/state.json");
        AppState::set_onboarding_demo_seen_at(&path, true).unwrap();
        assert!(path.exists());
        let state = AppState::load_at(&path).unwrap();
        assert!(state.onboarding.demo_seen);
    }
}
