use looprs::session_log::{SessionLogger, SessionEvent};
use tempfile::tempdir;
use std::io::BufRead;

#[test]
fn test_writes_valid_jsonl() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());
    logger.log(SessionEvent::UserMessage {
        content: "hello".into(),
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::SessionEnd);

    let path = logger.path();
    let file = std::fs::File::open(&path).unwrap();
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|l| l.unwrap())
        .collect();

    assert_eq!(lines.len(), 2);
    let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(first["event"], "user_message");
    assert_eq!(first["provider"], "ollama");
    assert!(first["ts"].is_string());
    assert!(first["session_id"].is_string());
}

#[test]
fn test_provider_tag_preserved() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());
    logger.log(SessionEvent::Inference {
        content: "response".into(),
        provider: "openai".into(),
    });
    let content = std::fs::read_to_string(logger.path()).unwrap();
    let event: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(event["provider"], "openai");
}

#[test]
fn test_all_event_types_serialize() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());

    logger.log(SessionEvent::UserMessage { content: "q".into(), provider: "ollama".into() });
    logger.log(SessionEvent::Inference { content: "a".into(), provider: "ollama".into() });
    logger.log(SessionEvent::ToolUse {
        tool_name: "bash".into(),
        input: serde_json::json!({"command": "ls"}),
        tool_use_id: "tu_1".into(),
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::ToolResult {
        tool_use_id: "tu_1".into(),
        output: "file.rs".into(),
        is_error: false,
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::SessionEnd);

    let path = logger.path();
    let lines: Vec<String> = std::io::BufReader::new(std::fs::File::open(path).unwrap())
        .lines().map(|l| l.unwrap()).collect();
    assert_eq!(lines.len(), 5);
    let event_types = ["user_message", "inference", "tool_use", "tool_result", "session_end"];
    for (i, expected) in event_types.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(&lines[i]).unwrap();
        assert_eq!(v["event"], *expected);
    }
}
