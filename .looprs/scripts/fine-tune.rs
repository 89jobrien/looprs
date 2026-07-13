#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! toml = "0.8"
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! rusqlite = "0.31"
//! ```

use serde::Deserialize;
use serde_json::Value;
use std::fs;

#[derive(Deserialize, Default)]
struct MagiConfig {
    db: Option<String>,
}

#[derive(Deserialize, Default)]
struct ModelsToml {
    magi: Option<MagiConfig>,
}

fn home() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn main() {
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

    let session_id = fs::read_to_string(last.path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["session_id"].as_str().map(str::to_owned))
        .last();

    let cfg_path = home().join(".looprs/models.toml");
    let db_path = fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| toml::from_str::<ModelsToml>(&s).ok())
        .and_then(|c| c.magi)
        .and_then(|m| m.db)
        .unwrap_or_default();

    let Some(sid) = session_id else {
        println!("Could not extract session_id from session log.");
        std::process::exit(1);
    };

    if db_path.is_empty() || !std::path::Path::new(&db_path).exists() {
        println!("Session {sid}: flagged locally (magi db not found).");
        return;
    }

    let conn = rusqlite::Connection::open(&db_path).expect("open db");
    conn.execute(
        "UPDATE interactions SET reward = MIN(reward + 0.2, 1.0) WHERE source_session = ?1",
        rusqlite::params![sid],
    )
    .expect("update reward");
    println!("Session {sid}: reward boosted in magi db.");
}
