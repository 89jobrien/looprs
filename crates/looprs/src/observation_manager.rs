use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::observation::Observation;
use crate::ports::ObservationStore;
use crate::types::ToolId;

/// Manages observation capture and storage across a session
pub struct ObservationManager {
    session_id: String,
    observations: Vec<Observation>,
}

impl ObservationManager {
    fn query_with_params(
        path: &std::path::Path,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> anyhow::Result<Vec<Observation>> {
        let conn = rusqlite::Connection::open(path)?;
        let mut stmt = conn.prepare(sql)?;
        let observations = stmt
            .query_map(params, |row| {
                let session_id: String = row.get(0)?;
                let tool_name: String = row.get(1)?;
                let input_str: String = row.get(2)?;
                let output: String = row.get(3)?;
                let tool_use_id_str: Option<String> = row.get(4)?;
                let timestamp: i64 = row.get(5)?;
                let context: Option<String> = row.get(6)?;

                Ok(crate::observation::Observation {
                    session_id,
                    tool_name,
                    input: serde_json::from_str(&input_str).unwrap_or(serde_json::Value::Null),
                    output,
                    tool_use_id: tool_use_id_str.map(ToolId::new),
                    timestamp: timestamp as u64,
                    context,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(observations)
    }

    fn limit_to_i64(limit: usize) -> i64 {
        i64::try_from(limit).unwrap_or(i64::MAX)
    }

    /// Persist all observations to a SQLite database at `path`.
    pub fn persist(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS observations (
                session_id TEXT NOT NULL,
                tool_name  TEXT NOT NULL,
                input      TEXT NOT NULL,
                output     TEXT NOT NULL,
                tool_use_id TEXT,
                timestamp  INTEGER NOT NULL,
                context    TEXT
            )",
        )?;
        for obs in &self.observations {
            conn.execute(
                "INSERT INTO observations
                 (session_id, tool_name, input, output, tool_use_id, timestamp, context)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    &obs.session_id,
                    &obs.tool_name,
                    obs.input.to_string(),
                    &obs.output,
                    obs.tool_use_id.as_ref().map(|id| id.as_str()),
                    obs.timestamp as i64,
                    obs.context.as_deref(),
                ],
            )?;
        }
        Ok(())
    }

    /// Load observations for `session_id` from a SQLite database at `path`.
    pub fn load_from(session_id: &str, path: &std::path::Path) -> anyhow::Result<Self> {
        let observations = Self::query_with_params(
            path,
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations WHERE session_id = ?1 ORDER BY timestamp ASC",
            &[&session_id],
        )?;
        Ok(Self {
            session_id: session_id.to_string(),
            observations,
        })
    }

    /// Query observations by tool name across sessions.
    pub fn query_by_tool(
        path: &std::path::Path,
        tool_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>> {
        let limit_i64 = Self::limit_to_i64(limit);
        Self::query_with_params(
            path,
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations
             WHERE tool_name = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
            &[&tool_name, &limit_i64],
        )
    }

    /// Query most recent observations across all sessions.
    pub fn query_recent(path: &std::path::Path, limit: usize) -> anyhow::Result<Vec<Observation>> {
        let limit_i64 = Self::limit_to_i64(limit);
        Self::query_with_params(
            path,
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations
             ORDER BY timestamp DESC
             LIMIT ?1",
            &[&limit_i64],
        )
    }

    /// Replay observations from another session into the current session cache.
    pub fn replay_from_session(
        &mut self,
        path: &std::path::Path,
        source_session_id: &str,
    ) -> anyhow::Result<usize> {
        let source = Self::load_from(source_session_id, path)?;
        let mut seen = self
            .observations
            .iter()
            .map(|obs| {
                (
                    obs.session_id.clone(),
                    obs.tool_name.clone(),
                    obs.timestamp,
                    obs.tool_use_id.as_ref().map(|id| id.as_str().to_string()),
                )
            })
            .collect::<HashSet<_>>();

        let mut replayed = 0usize;
        for obs in source.observations {
            let key = (
                obs.session_id.clone(),
                obs.tool_name.clone(),
                obs.timestamp,
                obs.tool_use_id.as_ref().map(|id| id.as_str().to_string()),
            );
            if seen.insert(key) {
                self.observations.push(obs);
                replayed = replayed.saturating_add(1);
            }
        }

        Ok(replayed)
    }

    /// Create a new observation manager for this session
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let session_id = format!("sess-{timestamp}");

        ObservationManager {
            session_id,
            observations: Vec::new(),
        }
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Capture a tool execution as an observation
    pub fn capture(
        &mut self,
        tool_name: String,
        input: Value,
        output: String,
        tool_use_id: Option<ToolId>,
    ) {
        let obs = Observation::new(
            tool_name,
            input,
            output,
            tool_use_id,
            self.session_id.clone(),
        );
        self.observations.push(obs);
    }

    /// Get all observations in this session
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    /// Count observations captured
    pub fn count(&self) -> usize {
        self.observations.len()
    }

    /// Save all observations via the given store.
    pub fn save(&self, store: &dyn ObservationStore) -> Result<()> {
        store.save(&self.observations)
    }

    /// Clear all observations (usually called after saving)
    pub fn clear(&mut self) {
        self.observations.clear();
    }
}

impl Default for ObservationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_persist_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");

        let mut mgr = ObservationManager::new();
        mgr.capture(
            "read".to_string(),
            serde_json::json!({"file": "foo.rs"}),
            "contents".to_string(),
            None,
        );
        mgr.persist(&path).unwrap();

        let loaded = ObservationManager::load_from(mgr.session_id(), &path).unwrap();
        assert_eq!(loaded.count(), 1);
        assert_eq!(loaded.observations()[0].tool_name, "read");
        assert_eq!(loaded.session_id(), mgr.session_id());
    }

    #[test]
    fn test_observation_manager_creation() {
        let mgr = ObservationManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.session_id().starts_with("sess-"));
    }

    #[test]
    fn test_observation_capture() {
        let mut mgr = ObservationManager::new();
        mgr.capture(
            "bash".to_string(),
            serde_json::json!({"command": "test"}),
            "output".to_string(),
            Some(ToolId::new("tool_1")),
        );

        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.observations()[0].tool_name, "bash");
        assert_eq!(
            mgr.observations()[0]
                .tool_use_id
                .as_ref()
                .map(|id| id.as_str()),
            Some("tool_1")
        );
    }

    #[test]
    fn test_multiple_observations() {
        let mut mgr = ObservationManager::new();
        mgr.capture(
            "bash".to_string(),
            serde_json::json!({}),
            "out1".to_string(),
            None,
        );
        mgr.capture(
            "grep".to_string(),
            serde_json::json!({}),
            "out2".to_string(),
            Some(ToolId::new("tool_2")),
        );

        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn test_observation_manager_clear() {
        let mut mgr = ObservationManager::new();
        mgr.capture(
            "bash".to_string(),
            serde_json::json!({}),
            "out".to_string(),
            None,
        );
        assert_eq!(mgr.count(), 1);

        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn query_by_tool_returns_cross_session_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");

        let mut first = ObservationManager::new();
        first.capture(
            "read".to_string(),
            serde_json::json!({"file": "one.rs"}),
            "one".to_string(),
            None,
        );
        first.persist(&path).unwrap();

        let mut second = ObservationManager::new();
        second.capture(
            "read".to_string(),
            serde_json::json!({"file": "two.rs"}),
            "two".to_string(),
            None,
        );
        second.persist(&path).unwrap();

        let rows = ObservationManager::query_by_tool(&path, "read", 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|obs| obs.tool_name == "read"));
    }

    #[test]
    fn replay_from_session_merges_previous_observations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");

        let mut source = ObservationManager::new();
        source.capture(
            "grep".to_string(),
            serde_json::json!({"pattern": "TODO"}),
            "match".to_string(),
            Some(ToolId::new("tool-replay")),
        );
        source.persist(&path).unwrap();

        let mut target = ObservationManager::new();
        let replayed = target
            .replay_from_session(&path, source.session_id())
            .unwrap();

        assert_eq!(replayed, 1);
        assert_eq!(target.count(), 1);
        assert_eq!(target.observations()[0].tool_name, "grep");
    }
}
