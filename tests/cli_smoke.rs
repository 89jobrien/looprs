use looprs::ApiConfig;
use std::sync::{Mutex, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    openrouter_api_key: Option<String>,
    anthropic_api_key: Option<String>,
}

impl EnvGuard {
    fn clear_keys() -> Self {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();
        let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();

        // SAFETY: env mutation is guarded by ENV_LOCK, ensuring exclusive access.
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        Self {
            _lock: lock,
            openrouter_api_key,
            anthropic_api_key,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: env mutation is guarded by ENV_LOCK, ensuring exclusive access.
        unsafe {
            match &self.openrouter_api_key {
                Some(value) => std::env::set_var("OPENROUTER_API_KEY", value),
                None => std::env::remove_var("OPENROUTER_API_KEY"),
            }

            match &self.anthropic_api_key {
                Some(value) => std::env::set_var("ANTHROPIC_API_KEY", value),
                None => std::env::remove_var("ANTHROPIC_API_KEY"),
            }
        }
    }
}

#[test]
fn api_config_requires_env() {
    let _guard = EnvGuard::clear_keys();

    match ApiConfig::from_env() {
        Ok(_) => panic!("expected missing API key error"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(msg.contains("No API key"));
        }
    }
}
