#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! serde = { version = "1", features = ["derive"] }
//! serde_yaml = "0.9"
//! ```

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Default)]
struct MagiConfig {
    modelcard: Option<String>,
}

#[derive(Deserialize, Default)]
struct ModelsToml {
    magi: Option<MagiConfig>,
}

#[derive(Deserialize, Default)]
struct EvalResult {
    mean_reward: Option<f64>,
}

#[derive(Deserialize, Default)]
struct ModelCard {
    model_id: Option<String>,
    training_status: Option<String>,
    eval_results: Option<HashMap<String, EvalResult>>,
}

fn main() {
    let cfg_path = dirs_next().join(".looprs/models.toml");
    let mc_path = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| toml::from_str::<ModelsToml>(&s).ok())
        .and_then(|c| c.magi)
        .and_then(|m| m.modelcard)
        .unwrap_or_default();

    if mc_path.is_empty() || !std::path::Path::new(&mc_path).exists() {
        println!("model: unknown (modelcard not found)");
        return;
    }

    let mc: ModelCard = std::fs::read_to_string(&mc_path)
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default();

    let model = mc.model_id.as_deref().unwrap_or("unknown");
    let status = mc.training_status.as_deref().unwrap_or("idle");
    let evals = mc.eval_results.unwrap_or_default();
    let rewards: Vec<f64> = evals.values().filter_map(|v| v.mean_reward).collect();
    let mean = if rewards.is_empty() {
        0.0
    } else {
        rewards.iter().sum::<f64>() / rewards.len() as f64
    };

    println!("model:   {model}");
    println!("status:  {status}");
    println!("reward:  {mean:.3} (mean across {} tasks)", rewards.len());
}

fn dirs_next() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}
