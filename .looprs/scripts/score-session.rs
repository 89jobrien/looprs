#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! rusqlite = "0.31"
//! ureq = "2"
//! ```

use serde::Deserialize;
use serde_json::{Value, json};
use std::{env, fs};

#[derive(Deserialize, Default)]
struct JudgeConfig {
    model: Option<String>,
}

#[derive(Deserialize, Default)]
struct TiersConfig {
    judge: Option<JudgeConfig>,
}

#[derive(Deserialize, Default)]
struct MagiConfig {
    db: Option<String>,
}

#[derive(Deserialize, Default)]
struct ModelsToml {
    tiers: Option<TiersConfig>,
    magi: Option<MagiConfig>,
}

fn home() -> std::path::PathBuf {
    env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| ".".into())
}

fn main() {
    let n: usize = env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let sessions_dir = home().join(".looprs/sessions");
    let mut entries: Vec<_> = fs::read_dir(&sessions_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .collect();
    entries.sort_by_key(|e| e.path());

    let Some(last) = entries.last() else {
        println!("No session logs found.");
        return;
    };

    let cfg_path = home().join(".looprs/models.toml");
    let cfg: ModelsToml = fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let scorer_model = cfg
        .tiers
        .as_ref()
        .and_then(|t| t.judge.as_ref())
        .and_then(|j| j.model.as_deref())
        .unwrap_or("gpt-4o")
        .to_owned();

    let db_path = cfg
        .magi
        .and_then(|m| m.db)
        .unwrap_or_default();

    let events: Vec<Value> = fs::read_to_string(last.path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|v: &Value| v["provider"].as_str() == Some("ollama"))
        .collect();

    let mut pairs: Vec<(String, String, String)> = Vec::new();
    let mut i = 0;
    while i + 1 < events.len() {
        if events[i]["event"].as_str() == Some("user_message")
            && events[i + 1]["event"].as_str() == Some("inference")
        {
            pairs.push((
                events[i]["content"].as_str().unwrap_or("").to_owned(),
                events[i + 1]["content"].as_str().unwrap_or("").to_owned(),
                events[i]["session_id"].as_str().unwrap_or("").to_owned(),
            ));
            i += 2;
        } else {
            i += 1;
        }
    }

    let pairs: Vec<_> = pairs.into_iter().rev().take(n).rev().collect();
    if pairs.is_empty() {
        println!("No ollama interactions found in session.");
        return;
    }

    let api_key = env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        println!("OPENAI_API_KEY not set — skipping scoring.");
        return;
    }

    println!("Scoring {} interactions with {scorer_model}...", pairs.len());

    let db_conn = if !db_path.is_empty() && std::path::Path::new(&db_path).exists() {
        rusqlite::Connection::open(&db_path).ok()
    } else {
        None
    };

    for (prompt, response, session_id) in &pairs {
        let body = json!({
            "model": scorer_model,
            "messages": [
                {"role": "system", "content": "Score this coding response 0.0-1.0. Reply: {\"score\": <float>}"},
                {"role": "user", "content": format!("Task: {prompt}\n\nResponse: {response}")}
            ],
            "temperature": 0.0
        });

        match ureq::post("https://api.openai.com/v1/chat/completions")
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Content-Type", "application/json")
            .send_json(&body)
        {
            Ok(resp) => {
                let data: Value = resp.into_json().unwrap_or_default();
                let content = data["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("{}");
                let score: f64 = serde_json::from_str::<Value>(content)
                    .ok()
                    .and_then(|v| v["score"].as_f64())
                    .unwrap_or(0.5);
                println!("  score={score:.3}  prompt={}", &prompt[..prompt.len().min(60)]);
                if let Some(conn) = &db_conn {
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO interactions \
                         (task, response, judge_score, reward, processed, source_session) \
                         VALUES (?1,?2,?3,?4,0,?5)",
                        rusqlite::params![prompt, response, score, score, session_id],
                    );
                }
            }
            Err(e) => println!("  scoring call failed: {e}"),
        }
    }
}
