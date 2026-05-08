use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const OBSERVABILITY_DIR_ENV: &str = "LOOPRS_OBSERVABILITY_DIR";

pub fn observability_root() -> PathBuf {
    std::env::var(OBSERVABILITY_DIR_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".looprs/observability"))
}

pub fn trace_dir() -> PathBuf {
    observability_root().join("traces")
}

pub fn append_named_jsonl(name: &str, value: &Value) -> io::Result<()> {
    let path = observability_root().join(format!("{name}.jsonl"));
    append_jsonl(&path, value)
}

pub fn append_jsonl(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut record = serde_json::Map::new();
    record.insert("ts".to_string(), Value::Number(now_millis().into()));
    record.insert("event".to_string(), value.clone());
    writeln!(
        file,
        "{}",
        serde_json::to_string(&Value::Object(record)).map_err(io::Error::other)?
    )?;
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
