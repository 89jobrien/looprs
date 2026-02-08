use looprs::ApiConfig;

struct EnvGuard {
    openrouter_api_key: Option<String>,
    anthropic_api_key: Option<String>,
}

impl EnvGuard {
    fn clear_keys() -> Self {
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();
        let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();

        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        Self {
            openrouter_api_key,
            anthropic_api_key,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.openrouter_api_key {
            Some(value) => unsafe {
                std::env::set_var("OPENROUTER_API_KEY", value);
            },
            None => unsafe {
                std::env::remove_var("OPENROUTER_API_KEY");
            },
        }

        match &self.anthropic_api_key {
            Some(value) => unsafe {
                std::env::set_var("ANTHROPIC_API_KEY", value);
            },
            None => unsafe {
                std::env::remove_var("ANTHROPIC_API_KEY");
            },
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
