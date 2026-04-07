use chrono::Utc;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    UserMessage { content: String, provider: String },
    Inference { content: String, provider: String },
    ToolUse {
        tool_name: String,
        input: serde_json::Value,
        tool_use_id: String,
        provider: String,
    },
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
        provider: String,
    },
    SessionEnd,
}

#[derive(Debug)]
pub struct SessionLogger {
    session_id: String,
    path: PathBuf,
}

impl SessionLogger {
    pub fn new(sessions_dir: PathBuf) -> Self {
        let session_id = format!("sess-{}", Uuid::new_v4());
        let date = Utc::now().format("%Y-%m-%d");
        let filename = format!("{}-{}.jsonl", date, session_id);
        fs::create_dir_all(&sessions_dir).ok();
        let path = sessions_dir.join(filename);
        Self { session_id, path }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn log(&mut self, event: SessionEvent) {
        #[derive(Serialize)]
        struct LogLine<'a> {
            ts: String,
            session_id: &'a str,
            #[serde(flatten)]
            event: &'a SessionEvent,
        }
        let line = LogLine {
            ts: Utc::now().to_rfc3339(),
            session_id: &self.session_id,
            event: &event,
        };
        if let Ok(mut json) = serde_json::to_string(&line) {
            json.push('\n');
            if let Ok(mut file) =
                OpenOptions::new().create(true).append(true).open(&self.path)
            {
                let _ = file.write_all(json.as_bytes());
            }
        }
    }
}
