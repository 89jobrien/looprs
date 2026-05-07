//! SqliteSessionStore adapter — `SessionStore` backed by a SQLite database.
//!
//! Each session is a row in `sessions`; each event is a row in `session_events`.
//! Uses `rusqlite` (already a workspace dep) — no async executor required since
//! `SessionStore::log` is synchronous.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use rusqlite::{Connection, params};
use uuid::Uuid;

use looprs_core::ports::session_store::{SessionEvent, SessionStore};

/// SQLite-backed session store.
///
/// Opens (or creates) a database at `db_path`. Each `SqliteSessionStore`
/// instance represents one session identified by a generated UUID.
pub struct SqliteSessionStore {
    session_id: String,
    db_path: PathBuf,
    conn: Connection,
}

impl SqliteSessionStore {
    /// Open the database at `db_path`, creating it if absent, and register a
    /// new session row.
    pub fn new(db_path: PathBuf) -> Result<Self, anyhow::Error> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create db dir: {}", parent.display()))?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open sqlite db: {}", db_path.display()))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id       TEXT PRIMARY KEY,
                started  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS session_events (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                ts         TEXT NOT NULL,
                event_kind TEXT NOT NULL,
                payload    TEXT NOT NULL
            );
            ",
        )
        .context("failed to initialise schema")?;

        let session_id = format!("sess-{}", Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, started) VALUES (?1, ?2)",
            params![session_id, now],
        )
        .context("failed to insert session row")?;

        Ok(Self {
            session_id,
            db_path,
            conn,
        })
    }

    /// Open against an in-memory database. Useful for tests.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self, anyhow::Error> {
        let conn = Connection::open_in_memory().context("failed to open in-memory db")?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id       TEXT PRIMARY KEY,
                started  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS session_events (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                ts         TEXT NOT NULL,
                event_kind TEXT NOT NULL,
                payload    TEXT NOT NULL
            );
            ",
        )?;
        let session_id = format!("sess-{}", Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, started) VALUES (?1, ?2)",
            params![session_id, now],
        )?;
        Ok(Self {
            session_id,
            db_path: PathBuf::from(":memory:"),
            conn,
        })
    }
}

impl SessionStore for SqliteSessionStore {
    fn log(&mut self, event: SessionEvent) -> Result<(), anyhow::Error> {
        let kind = match &event {
            SessionEvent::UserMessage { .. } => "user_message",
            SessionEvent::Inference { .. } => "inference",
            SessionEvent::ToolUse { .. } => "tool_use",
            SessionEvent::ToolResult { .. } => "tool_result",
            SessionEvent::SessionEnd => "session_end",
        };
        let payload = serde_json::to_string(&event).context("failed to serialize event")?;
        let ts = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO session_events (session_id, ts, event_kind, payload)
                 VALUES (?1, ?2, ?3, ?4)",
                params![self.session_id, ts, kind, payload],
            )
            .context("failed to insert session event")?;
        Ok(())
    }

    fn path(&self) -> Option<&Path> {
        if self.db_path == PathBuf::from(":memory:") {
            None
        } else {
            Some(&self.db_path)
        }
    }

    fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_all_event_kinds() {
        let mut store = SqliteSessionStore::in_memory().unwrap();

        store
            .log(SessionEvent::UserMessage {
                content: "hello".into(),
                provider: "mock".into(),
            })
            .unwrap();
        store
            .log(SessionEvent::Inference {
                content: "world".into(),
                provider: "mock".into(),
            })
            .unwrap();
        store
            .log(SessionEvent::ToolUse {
                tool_name: "bash".into(),
                input: serde_json::json!({"cmd": "echo hi"}),
                tool_use_id: "tu-1".into(),
                provider: "mock".into(),
            })
            .unwrap();
        store
            .log(SessionEvent::ToolResult {
                tool_use_id: "tu-1".into(),
                output: "hi".into(),
                is_error: false,
                provider: "mock".into(),
            })
            .unwrap();
        store.log(SessionEvent::SessionEnd).unwrap();

        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
                params![store.session_id],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(count, 5);
    }

    #[test]
    fn session_id_is_stable() {
        let store = SqliteSessionStore::in_memory().unwrap();
        let id = store.session_id().to_string();
        assert_eq!(store.session_id(), id);
    }

    #[test]
    fn in_memory_path_returns_none() {
        let store = SqliteSessionStore::in_memory().unwrap();
        assert!(store.path().is_none());
    }

    #[test]
    fn on_disk_path_returns_some() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.sqlite3");
        let store = SqliteSessionStore::new(db.clone()).unwrap();
        assert_eq!(store.path().unwrap(), db);
    }

    #[test]
    fn event_payload_is_valid_json() {
        let mut store = SqliteSessionStore::in_memory().unwrap();
        store
            .log(SessionEvent::UserMessage {
                content: "test".into(),
                provider: "mock".into(),
            })
            .unwrap();

        let payload: String = store
            .conn
            .query_row(
                "SELECT payload FROM session_events WHERE session_id = ?1",
                params![store.session_id],
                |r| r.get(0),
            )
            .unwrap();

        let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(v["event"], "user_message");
        assert_eq!(v["content"], "test");
    }
}
