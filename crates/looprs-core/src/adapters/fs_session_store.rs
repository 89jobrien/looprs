//! FsSessionStore adapter — `SessionStore` backed by a JSONL file on disk.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::ports::session_store::{SessionEvent, SessionStore};

/// Filesystem-backed session store that appends JSONL lines to a dated log file.
pub struct FsSessionStore {
    session_id: String,
    path: PathBuf,
}

impl FsSessionStore {
    /// Create a new store under `sessions_dir`, generating a unique session id.
    pub fn new(sessions_dir: PathBuf) -> Result<Self, anyhow::Error> {
        let session_id = format!("sess-{}", Uuid::new_v4());
        let date = Utc::now().format("%Y-%m-%d");
        let filename = format!("{}-{}.jsonl", date, session_id);
        fs::create_dir_all(&sessions_dir).with_context(|| {
            format!("failed to create sessions dir: {}", sessions_dir.display())
        })?;
        let path = sessions_dir.join(filename);
        Ok(Self { session_id, path })
    }
}

impl SessionStore for FsSessionStore {
    fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error> {
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

    fn path(&self) -> Option<&Path> {
        Some(&self.path)
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conformance() {
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let mut store =
            FsSessionStore::new(dir.path().join("sessions")).expect("failed to create store");
        crate::ports::test_contracts::assert_session_store_contract(&mut store);
    }
}
