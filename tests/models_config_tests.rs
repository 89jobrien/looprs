use looprs::models_config::ModelsConfig;
use std::io::Write;
use tempfile::NamedTempFile;

fn write_toml(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

#[test]
fn test_parse_full_config() {
    let toml = r#"
[default]
provider = "ollama"
model = "magistral-small-rl-v17"

[tiers]
fast      = { provider = "ollama", model = "qwen2.5-coder:7b" }
capable   = { provider = "ollama", model = "magistral-small-rl-v17" }
outsource = { provider = "openai", model = "gpt-4o" }
judge     = { provider = "openai", model = "gpt-5.4" }

[magi]
modelcard = "/dev/magi/modelcard.yaml"
db        = "/dev/magi/db/rewards.db"
"#;
    let f = write_toml(toml);
    let config = ModelsConfig::from_path(f.path()).unwrap();
    assert_eq!(config.default.provider, "ollama");
    assert_eq!(config.default.model, "magistral-small-rl-v17");
    assert_eq!(config.tier("outsource").unwrap().provider, "openai");
    assert_eq!(config.tier("judge").unwrap().model, "gpt-5.4");
    assert_eq!(config.magi_modelcard(), "/dev/magi/modelcard.yaml");
    assert_eq!(config.magi_db(), "/dev/magi/db/rewards.db");
}

#[test]
fn test_missing_tier_returns_none() {
    let toml = r#"
[default]
provider = "ollama"
model = "qwen2.5-coder:7b"
"#;
    let f = write_toml(toml);
    let config = ModelsConfig::from_path(f.path()).unwrap();
    assert!(config.tier("outsource").is_none());
    assert!(config.magi_modelcard().is_empty());
}
