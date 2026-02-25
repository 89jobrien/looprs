use anyhow::Result;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::providers::{InferenceRequest, InferenceResponse};

pub fn append_turn_trace(
    session_id: &str,
    request: &InferenceRequest,
    response: &InferenceResponse,
) -> Result<()> {
    append_turn_trace_in_dir(Path::new("."), session_id, request, response)
}

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

    let trace_dir = base_dir.join(".looprs").join("traces");
    fs::create_dir_all(&trace_dir)?;

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

pub fn session_trace_path(base_dir: &Path, session_id: &str) -> PathBuf {
    base_dir
        .join(".looprs")
        .join("traces")
        .join(format!("{session_id}.jsonl"))
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
}
