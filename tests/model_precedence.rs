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

#[tokio::test]
async fn provider_json_model_used_when_no_env_or_override() {
    let _guard = EnvDirGuard::new();
    // SAFETY: env mutation is guarded by TEST_LOCK.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test");
    }

    let tmp = TempDir::new().unwrap();
    write_provider_json(
        &tmp,
        r#"{
  "provider": "openai",
  "openai": { "model": "gpt-5-mini" }
}"#,
    );

    std::env::set_current_dir(tmp.path()).unwrap();

    let provider = create_provider_with_overrides(ProviderOverrides::default())
        .await
        .unwrap();

    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model().as_str(), "gpt-5-mini");
}

#[tokio::test]
async fn env_model_overrides_provider_json() {
    let _guard = EnvDirGuard::new();
    // SAFETY: env mutation is guarded by TEST_LOCK.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test");
        std::env::set_var("MODEL", "gpt-4o-mini");
    }

    let tmp = TempDir::new().unwrap();
    write_provider_json(
        &tmp,
        r#"{
  "provider": "openai",
  "openai": { "model": "gpt-5-mini" }
}"#,
    );
    std::env::set_current_dir(tmp.path()).unwrap();

    let provider = create_provider_with_overrides(ProviderOverrides::default())
        .await
        .unwrap();

    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model().as_str(), "gpt-4o-mini");
}

#[tokio::test]
async fn overrides_model_overrides_env_and_provider_json() {
    let _guard = EnvDirGuard::new();
    // SAFETY: env mutation is guarded by TEST_LOCK.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test");
        std::env::set_var("MODEL", "gpt-4o-mini");
    }

    let tmp = TempDir::new().unwrap();
    write_provider_json(
        &tmp,
        r#"{
  "provider": "openai",
  "openai": { "model": "gpt-5-mini" }
}"#,
    );
    std::env::set_current_dir(tmp.path()).unwrap();

    let provider = create_provider_with_overrides(ProviderOverrides {
        model: Some(ModelId::new("gpt-5-mini-override")),
    })
    .await
    .unwrap();

    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model().as_str(), "gpt-5-mini-override");
}

#[tokio::test]
async fn openai_default_model_is_gpt_5_mini() {
    let _guard = EnvDirGuard::new();
    // SAFETY: env mutation is guarded by TEST_LOCK.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test");
        std::env::set_var("PROVIDER", "openai");
    }

    // No provider.json and no MODEL env should fall back to provider default.
    let tmp = TempDir::new().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();

    let provider = create_provider_with_overrides(ProviderOverrides::default())
        .await
        .unwrap();

    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.model().as_str(), "gpt-5-mini");
}
