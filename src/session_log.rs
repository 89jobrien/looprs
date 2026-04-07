use anyhow::Context as _;
use chrono::Utc;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Represents a discrete event that can be recorded in a session log.
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
    /// Creates a new `SessionLogger`, ensuring `sessions_dir` exists.
    pub fn new(sessions_dir: PathBuf) -> Result<Self, anyhow::Error> {
        let session_id = format!("sess-{}", Uuid::new_v4());
        let date = Utc::now().format("%Y-%m-%d");
        let filename = format!("{}-{}.jsonl", date, session_id);
        fs::create_dir_all(&sessions_dir)
            .with_context(|| format!("failed to create sessions dir: {}", sessions_dir.display()))?;
        let path = sessions_dir.join(filename);
        Ok(Self { session_id, path })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends a `SessionEvent` as a JSONL line to the session log file.
    pub fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error> {
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
        let mut json = serde_json::to_string(&line).context("failed to serialize log line")?;
        json.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open log file: {}", self.path.display()))?;
        file.write_all(json.as_bytes())
            .context("failed to write log line")?;
        Ok(())
    }
}
