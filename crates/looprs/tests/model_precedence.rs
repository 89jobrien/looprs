use std::sync::{Mutex, OnceLock};

use tempfile::TempDir;

use looprs::{ModelId, ProviderOverrides, create_provider_with_overrides};

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvDirGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    cwd: std::path::PathBuf,
    openai_api_key: Option<String>,
    anthropic_api_key: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

impl EnvDirGuard {
    fn new() -> Self {
        let lock = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let cwd = std::env::current_dir().unwrap();

        let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
        let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let provider = std::env::var("PROVIDER").ok();
        let model = std::env::var("MODEL").ok();

        // SAFETY: env mutation is guarded by TEST_LOCK.
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("PROVIDER");
            std::env::remove_var("MODEL");
        }

        Self {
            _lock: lock,
            cwd,
            openai_api_key,
            anthropic_api_key,
            provider,
            model,
        }
    }
}

impl Drop for EnvDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.cwd);

        // SAFETY: env mutation is guarded by TEST_LOCK.
        unsafe {
            match &self.openai_api_key {
                Some(v) => std::env::set_var("OPENAI_API_KEY", v),
                None => std::env::remove_var("OPENAI_API_KEY"),
            }
            match &self.anthropic_api_key {
                Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
            match &self.provider {
                Some(v) => std::env::set_var("PROVIDER", v),
                None => std::env::remove_var("PROVIDER"),
            }
            match &self.model {
                Some(v) => std::env::set_var("MODEL", v),
                None => std::env::remove_var("MODEL"),
            }
        }
    }
}

fn write_provider_json(dir: &TempDir, json: &str) {
    let looprs_dir = dir.path().join(".looprs");
    std::fs::create_dir_all(&looprs_dir).unwrap();
    std::fs::write(looprs_dir.join("provider.json"), json).unwrap();
}

const OPENAI_PROVIDER_JSON: &str = r#"{
  "provider": "openai",
  "openai": { "model": "gpt-5-mini" }
}"#;

/// Set up a test scenario: env vars, optional provider.json, optional overrides,
/// then assert provider name and model.
async fn assert_model_precedence(
    env_vars: &[(&str, &str)],
    provider_json: Option<&str>,
    overrides: ProviderOverrides,
    expected_provider: &str,
    expected_model: &str,
) {
    let _guard = EnvDirGuard::new();
    // SAFETY: env mutation is guarded by TEST_LOCK.
    unsafe {
        for (k, v) in env_vars {
            std::env::set_var(k, v);
        }
    }

    let tmp = TempDir::new().unwrap();
    if let Some(json) = provider_json {
        write_provider_json(&tmp, json);
    }
    std::env::set_current_dir(tmp.path()).unwrap();

    let provider = create_provider_with_overrides(overrides).await.unwrap();
    assert_eq!(provider.name(), expected_provider);
    assert_eq!(provider.model().as_str(), expected_model);
}

#[tokio::test]
async fn provider_json_model_used_when_no_env_or_override() {
    assert_model_precedence(
        &[("OPENAI_API_KEY", "test")],
        Some(OPENAI_PROVIDER_JSON),
        ProviderOverrides::default(),
        "openai",
        "gpt-5-mini",
    )
    .await;
}

#[tokio::test]
async fn env_model_overrides_provider_json() {
    assert_model_precedence(
        &[("OPENAI_API_KEY", "test"), ("MODEL", "gpt-4o-mini")],
        Some(OPENAI_PROVIDER_JSON),
        ProviderOverrides::default(),
        "openai",
        "gpt-4o-mini",
    )
    .await;
}

#[tokio::test]
async fn overrides_model_overrides_env_and_provider_json() {
    assert_model_precedence(
        &[("OPENAI_API_KEY", "test"), ("MODEL", "gpt-4o-mini")],
        Some(OPENAI_PROVIDER_JSON),
        ProviderOverrides {
            model: Some(ModelId::new("gpt-5-mini-override")),
        },
        "openai",
        "gpt-5-mini-override",
    )
    .await;
}

#[tokio::test]
async fn openai_default_model_is_gpt_5_mini() {
    assert_model_precedence(
        &[("OPENAI_API_KEY", "test"), ("PROVIDER", "openai")],
        None,
        ProviderOverrides::default(),
        "openai",
        "gpt-5-mini",
    )
    .await;
}
