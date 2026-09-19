use anyhow::Result;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::observability;
use crate::providers::{InferenceRequest, InferenceResponse};

/// Append one inference turn to the configured session JSONL trace.
pub fn append_turn_trace(
    session_id: &str,
    request: &InferenceRequest,
    response: &InferenceResponse,
) -> Result<()> {
    let base = observability::trace_dir();
    append_turn_trace_in_dir(base.as_path(), session_id, request, response)
}

/// Append one inference turn to `<base_dir>/<session_id>.jsonl`.
///
/// Parent directories are created as needed. Existing records are preserved.
pub fn append_turn_trace_in_dir(
    base_dir: &Path,
    session_id: &str,
    request: &InferenceRequest,
    response: &InferenceResponse,
) -> Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let trace_dir = base_dir;
    fs::create_dir_all(trace_dir)?;

    let trace_file = trace_dir.join(format!("{session_id}.jsonl"));

    let record = json!({
        "timestamp": timestamp,
        "session_id": session_id,
        "turn": {
            "request": {
                "model": request.model.as_str(),
                "messages": &request.messages,
                "tools": &request.tools,
                "max_tokens": request.max_tokens,
                "temperature": request.temperature,
                "system": &request.system,
            },
            "response": {
                "content": &response.content,
                "stop_reason": &response.stop_reason,
                "usage": &response.usage,
            }
        }
    });

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&trace_file)?;
    writeln!(file, "{}", serde_json::to_string(&record)?)?;

    Ok(())
}

/// Build the JSONL trace path for a session.
///
/// ```
/// use std::path::Path;
/// use looprs::session_trace_path;
///
/// assert_eq!(
///     session_trace_path(Path::new("traces"), "session-1"),
///     Path::new("traces/session-1.jsonl")
/// );
/// ```
pub fn session_trace_path(base_dir: &Path, session_id: &str) -> PathBuf {
    base_dir.join(format!("{session_id}.jsonl"))
}

/// Return whether trace data is older than `repository_activity`.
///
/// Missing directories, unreadable `.jsonl` entries, malformed JSON records,
/// and invalid timestamps all fail stale (`Ok(true)`). Non-JSONL entries are
/// ignored. Across valid files, the newest record determines freshness.
///
/// ```
/// use std::path::Path;
/// use std::time::{Duration, UNIX_EPOCH};
/// use looprs::trace_stream_is_stale;
///
/// let stale = trace_stream_is_stale(
///     Path::new("definitely-missing-traces"),
///     UNIX_EPOCH + Duration::from_secs(1),
/// )?;
/// assert!(stale);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn trace_stream_is_stale(base_dir: &Path, repository_activity: SystemTime) -> Result<bool> {
    let activity_timestamp = match repository_activity.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => return Ok(true),
    };
    let entries = match fs::read_dir(base_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(true),
    };
    let mut latest_timestamp = None;

    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(_) => return Ok(true),
        };
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => return Ok(true),
        };
        for line in content.lines() {
            let record = match serde_json::from_str::<serde_json::Value>(line) {
                Ok(record) => record,
                Err(_) => return Ok(true),
            };
            let Some(timestamp) = record.get("timestamp").and_then(serde_json::Value::as_u64)
            else {
                return Ok(true);
            };
            latest_timestamp =
                Some(latest_timestamp.map_or(timestamp, |latest: u64| latest.max(timestamp)));
        }
    }

    Ok(latest_timestamp.is_none_or(|timestamp| timestamp < activity_timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ContentBlock, Message, ToolDefinition};
    use crate::providers::Usage;
    use crate::types::{ModelId, ToolId, ToolName};
    use tempfile::TempDir;

    #[test]
    fn append_turn_trace_writes_jsonl_record() {
        let temp = TempDir::new().expect("tempdir");
        let req = InferenceRequest {
            model: ModelId::new("mock-model"),
            messages: vec![Message::user("hello")],
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read file".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            max_tokens: 1024,
            temperature: Some(0.2),
            system: "system prompt".to_string(),
        };
        let resp = InferenceResponse {
            content: vec![ContentBlock::ToolUse {
                id: ToolId::new("tool_1"),
                name: ToolName::new("read"),
                input: json!({"path": "README.md"}),
            }],
            stop_reason: "tool_use".to_string(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 4,
            },
        };

        append_turn_trace_in_dir(temp.path(), "sess-42", &req, &resp).expect("trace append");

        let trace_file = session_trace_path(temp.path(), "sess-42");
        let content = std::fs::read_to_string(&trace_file).expect("trace file content");
        let line = content.lines().next().expect("jsonl first line");
        let parsed: serde_json::Value = serde_json::from_str(line).expect("parse json line");

        assert_eq!(parsed["session_id"], "sess-42");
        assert_eq!(parsed["turn"]["request"]["model"], "mock-model");
        assert_eq!(parsed["turn"]["response"]["stop_reason"], "tool_use");
    }

    #[test]
    fn trace_stream_without_records_is_stale() {
        let temp = TempDir::new().expect("tempdir");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(10))
                .expect("staleness check")
        );
    }

    #[test]
    fn trace_stream_pre_epoch_repository_activity_fails_stale() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(
            session_trace_path(temp.path(), "session"),
            "{\"timestamp\":1}\n",
        )
        .expect("trace fixture");
        let repository_activity = UNIX_EPOCH
            .checked_sub(std::time::Duration::from_secs(1))
            .expect("pre-epoch timestamp");

        assert!(
            trace_stream_is_stale(temp.path(), repository_activity)
                .expect("pre-epoch repository activity")
        );
    }

    #[test]
    fn trace_stream_detects_activity_newer_than_latest_record() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(
            session_trace_path(temp.path(), "session"),
            "{\"timestamp\":100}\n{\"timestamp\":200}\n",
        )
        .expect("trace fixture");

        assert!(
            trace_stream_is_stale(
                temp.path(),
                UNIX_EPOCH + std::time::Duration::from_secs(201)
            )
            .expect("staleness check")
        );
        assert!(
            !trace_stream_is_stale(
                temp.path(),
                UNIX_EPOCH + std::time::Duration::from_secs(200)
            )
            .expect("freshness check")
        );
    }

    #[test]
    fn trace_stream_missing_directory_is_stale() {
        let temp = TempDir::new().expect("tempdir");
        let missing = temp.path().join("missing");

        assert!(
            trace_stream_is_stale(&missing, UNIX_EPOCH + std::time::Duration::from_secs(1))
                .expect("missing trace directory")
        );
    }

    #[test]
    fn trace_stream_unreadable_jsonl_entry_fails_stale() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::create_dir(temp.path().join("blocked.jsonl")).expect("directory fixture");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(1))
                .expect("unreadable trace")
        );
    }

    #[test]
    fn trace_stream_malformed_json_fails_stale() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(session_trace_path(temp.path(), "broken"), "not-json\n")
            .expect("trace fixture");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(1))
                .expect("malformed trace")
        );
    }

    #[test]
    fn trace_stream_invalid_timestamp_fails_stale() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(
            session_trace_path(temp.path(), "broken"),
            "{\"timestamp\":\"recent\"}\n",
        )
        .expect("trace fixture");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(1))
                .expect("invalid timestamp")
        );
    }

    #[test]
    fn trace_stream_ignores_non_jsonl_files() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(temp.path().join("notes.txt"), "{\"timestamp\":999}\n")
            .expect("non-trace fixture");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(1))
                .expect("non-jsonl file")
        );
    }

    #[test]
    fn trace_stream_uses_latest_timestamp_across_multiple_files() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(
            session_trace_path(temp.path(), "older"),
            "{\"timestamp\":10}\n",
        )
        .expect("older trace");
        std::fs::write(
            session_trace_path(temp.path(), "newer"),
            "{\"timestamp\":20}\n",
        )
        .expect("newer trace");

        assert!(
            !trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(20))
                .expect("multiple trace files")
        );
    }

    #[test]
    fn trace_stream_malformed_file_overrides_fresh_file() {
        let temp = TempDir::new().expect("tempdir");
        std::fs::write(
            session_trace_path(temp.path(), "fresh"),
            "{\"timestamp\":20}\n",
        )
        .expect("fresh trace");
        std::fs::write(session_trace_path(temp.path(), "broken"), "not-json\n")
            .expect("broken trace");

        assert!(
            trace_stream_is_stale(temp.path(), UNIX_EPOCH + std::time::Duration::from_secs(20))
                .expect("fail-stale trace set")
        );
    }
}
