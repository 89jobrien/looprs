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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::sync::Mutex;

    // Mutex to serialize env var tests to avoid race conditions
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_observability_root_default_when_env_not_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }

        let root = observability_root();
        assert_eq!(root, PathBuf::from(".looprs/observability"));
    }

    #[test]
    fn test_observability_root_custom_when_env_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let custom_path = "/tmp/test_custom_observability_12345";
        unsafe { std::env::set_var(OBSERVABILITY_DIR_ENV, custom_path) }

        let root = observability_root();
        assert_eq!(root, PathBuf::from(custom_path));

        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }
    }

    #[test]
    fn test_observability_root_ignores_empty_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var(OBSERVABILITY_DIR_ENV, "  ") }

        let root = observability_root();
        assert_eq!(root, PathBuf::from(".looprs/observability"));

        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }
    }

    #[test]
    fn test_trace_dir_appends_traces() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }

        let trace = trace_dir();
        assert_eq!(trace, PathBuf::from(".looprs/observability/traces"));
    }

    #[test]
    fn test_trace_dir_with_custom_root() {
        let _guard = ENV_LOCK.lock().unwrap();
        let custom_path = "/tmp/test_custom_trace_67890";
        unsafe { std::env::set_var(OBSERVABILITY_DIR_ENV, custom_path) }

        let trace = trace_dir();
        assert_eq!(trace, PathBuf::from("/tmp/test_custom_trace_67890/traces"));

        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }
    }

    #[test]
    fn test_append_jsonl_creates_file_with_valid_format() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let temp_path = temp_dir.path().join("test_events.jsonl");

        let event = json!({"action": "test_action", "count": 42});
        append_jsonl(&temp_path, &event).expect("failed to append jsonl");

        // Verify file exists and contains valid JSON
        let content = fs::read_to_string(&temp_path).expect("failed to read file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "should have exactly 1 line");

        let record: serde_json::Value =
            serde_json::from_str(lines[0]).expect("line should be valid JSON");

        assert!(record.get("ts").is_some(), "should have ts field");
        assert!(record.get("event").is_some(), "should have event field");

        let recorded_event = &record["event"];
        assert_eq!(recorded_event["action"], "test_action");
        assert_eq!(recorded_event["count"], 42);
    }

    #[test]
    fn test_append_jsonl_appends_multiple_records() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let temp_path = temp_dir.path().join("test_events.jsonl");

        let event1 = json!({"msg": "first"});
        let event2 = json!({"msg": "second"});

        append_jsonl(&temp_path, &event1).expect("failed to append first jsonl");
        append_jsonl(&temp_path, &event2).expect("failed to append second jsonl");

        // Verify both records exist
        let content = fs::read_to_string(&temp_path).expect("failed to read file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "should have exactly 2 lines");

        let record1: serde_json::Value =
            serde_json::from_str(lines[0]).expect("first line should be valid JSON");
        let record2: serde_json::Value =
            serde_json::from_str(lines[1]).expect("second line should be valid JSON");

        assert_eq!(record1["event"]["msg"], "first");
        assert_eq!(record2["event"]["msg"], "second");
    }

    #[test]
    fn test_append_jsonl_creates_parent_directories() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let nested_path = temp_dir
            .path()
            .join("nested")
            .join("deep")
            .join("path")
            .join("events.jsonl");

        let event = json!({"test": "nested"});
        append_jsonl(&nested_path, &event).expect("failed to append jsonl");

        assert!(nested_path.exists(), "file should exist");
        assert!(
            nested_path.parent().unwrap().exists(),
            "parent dirs should exist"
        );

        let content = fs::read_to_string(&nested_path).expect("failed to read file");
        assert!(!content.is_empty(), "file should not be empty");
    }

    #[test]
    fn test_append_named_jsonl_creates_correctly_named_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        unsafe { std::env::set_var(OBSERVABILITY_DIR_ENV, temp_dir.path()) }

        let event = json!({"data": "test"});
        append_named_jsonl("test_events", &event).expect("failed to append named jsonl");

        let expected_file = temp_dir.path().join("test_events.jsonl");
        assert!(
            expected_file.exists(),
            "file with correct name should exist"
        );

        // Verify content
        let content = fs::read_to_string(&expected_file).expect("failed to read file");
        let record: serde_json::Value =
            serde_json::from_str(content.trim()).expect("content should be valid JSON");

        assert_eq!(record["event"]["data"], "test");

        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }
    }

    #[test]
    fn test_append_named_jsonl_respects_custom_root() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        unsafe { std::env::set_var(OBSERVABILITY_DIR_ENV, temp_dir.path()) }

        let event1 = json!({"seq": 1});
        let event2 = json!({"seq": 2});

        append_named_jsonl("events", &event1).expect("failed to append first");
        append_named_jsonl("events", &event2).expect("failed to append second");

        let events_file = temp_dir.path().join("events.jsonl");
        let content = fs::read_to_string(&events_file).expect("failed to read file");
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2, "should have 2 events in the file");

        unsafe { std::env::remove_var(OBSERVABILITY_DIR_ENV) }
    }

    #[test]
    fn test_append_jsonl_preserves_json_structure() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let temp_path = temp_dir.path().join("test_events.jsonl");

        let complex_event = json!({
            "action": "user_login",
            "user_id": 123,
            "timestamp": "2024-01-01T12:00:00Z",
            "metadata": {
                "ip": "192.168.1.1",
                "tags": ["admin", "active"]
            }
        });

        append_jsonl(&temp_path, &complex_event).expect("failed to append jsonl");

        let content = fs::read_to_string(&temp_path).expect("failed to read file");
        let record: serde_json::Value =
            serde_json::from_str(content.trim()).expect("should be valid JSON");

        let event = &record["event"];
        assert_eq!(event["action"], "user_login");
        assert_eq!(event["user_id"], 123);
        assert_eq!(event["metadata"]["ip"], "192.168.1.1");
        assert_eq!(event["metadata"]["tags"][0], "admin");
    }
}
