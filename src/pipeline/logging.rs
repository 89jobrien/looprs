use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PipelineLogger {
    run_id: Option<String>,
    file: Mutex<File>,
}

impl PipelineLogger {
    pub fn new(log_dir: PathBuf) -> io::Result<Self> {
        create_dir_all(&log_dir)?;
        let path = log_dir.join("events.jsonl");
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            run_id: None,
            file: Mutex::new(file),
        })
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    /// Note: `flush` only pushes to OS buffers; it does not guarantee durability on disk.
    /// Use `sync_data`/`sync_all` (or a future config) if fsync-level durability is required.
    pub fn log_event(&self, step: &str, data: serde_json::Value) -> io::Result<()> {
        let mut event = serde_json::Map::new();
        event.insert(
            "step".to_string(),
            serde_json::Value::String(step.to_string()),
        );
        event.insert("data".to_string(), data);

        if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
            let millis = duration.as_millis() as u64;
            event.insert("ts".to_string(), serde_json::Value::Number(millis.into()));
        }

        if let Some(run_id) = &self.run_id {
            event.insert(
                "run_id".to_string(),
                serde_json::Value::String(run_id.clone()),
            );
        }

        let line = serde_json::to_string(&event).map_err(io::Error::other)?;
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("logger mutex poisoned"))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineLogger;

    #[test]
    fn test_jsonl_event_written() {
        let dir = tempfile::tempdir().unwrap();
        let logger = PipelineLogger::new(dir.path().to_path_buf()).unwrap();
        logger
            .log_event("test", serde_json::json!({"ok": true}))
            .unwrap();
        let entries = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(entries.contains("\"step\":\"test\""));
    }
}
