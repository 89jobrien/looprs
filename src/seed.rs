//! Seed example config files into a directory. Never overwrites user config.

use anyhow::Result;
use std::path::Path;

use crate::app_config::AppConfig;
use crate::config_file::ProviderConfig;

/// Write example config files into `dir`. Creates `config.json.example` and
/// `provider.json.example`. Does not overwrite existing `config.json` or `provider.json`.
pub fn seed_into(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::new();

    let config_example = dir.join("config.json.example");
    let content = serde_json::to_string_pretty(&AppConfig::default())?;
    std::fs::write(&config_example, content)?;
    written.push(config_example);

    let provider_example = dir.join("provider.json.example");
    let content = serde_json::to_string_pretty(&ProviderConfig::default())?;
    std::fs::write(&provider_example, content)?;
    written.push(provider_example);

    Ok(written)
}

/// Expand leading `~` to home directory.
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if path == "~" || path.starts_with("~/") {
        if let Some(home) = std::env::home_dir() {
            if path == "~" {
                return home;
            }
            return home.join(path.trim_start_matches("~/"));
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn seed_creates_example_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(".looprs");
        let written = seed_into(&dir).unwrap();
        assert_eq!(written.len(), 2);
        assert!(dir.join("config.json.example").exists());
        assert!(dir.join("provider.json.example").exists());
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("config.json.example")).unwrap(),
        )
        .unwrap();
        assert!(config.get("defaults").is_some());
        assert!(config.get("onboarding").is_some());
    }

    #[test]
    fn expand_tilde_plain_path_unchanged() {
        let p = expand_tilde("/foo/bar");
        assert_eq!(p, std::path::Path::new("/foo/bar"));
    }
}
