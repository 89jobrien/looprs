use agent_domain::{TracePort, TraceStep};
use async_trait::async_trait;
use rusqlite::Connection;
use std::sync::Mutex;

/// SQLite-backed reasoning-trace adapter.
///
/// Uses `rusqlite` with `Mutex` for interior mutability so the adapter is
/// `Send + Sync` and can be shared across async tasks.
pub struct SqliteTraceAdapter {
    conn: Mutex<Connection>,
}

impl SqliteTraceAdapter {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trace_steps (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 trace_id    TEXT    NOT NULL,
                 step_index  INTEGER NOT NULL,
                 data        TEXT    NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_trace_id
                 ON trace_steps (trace_id, step_index);",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

#[async_trait]
impl TracePort for SqliteTraceAdapter {
    async fn append_step(&self, step: TraceStep) -> anyhow::Result<()> {
        let json = serde_json::to_string(&step)?;
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "INSERT INTO trace_steps (trace_id, step_index, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![step.trace_id, step.step_index, json],
        )?;
        Ok(())
    }

    async fn load_trace(&self, trace_id: &str) -> anyhow::Result<Vec<TraceStep>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT data FROM trace_steps WHERE trace_id = ?1 ORDER BY step_index",
        )?;
        let rows = stmt.query_map(rusqlite::params![trace_id], |row| {
            row.get::<_, String>(0)
        })?;

        let mut out = Vec::new();
        for row in rows {
            let json_str = row?;
            let step: TraceStep = serde_json::from_str(&json_str)?;
            out.push(step);
        }
        Ok(out)
    }
}
