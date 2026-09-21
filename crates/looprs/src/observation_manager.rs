use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::SqliteObservationStore;
use crate::observation::Observation;
use crate::ports::{ObservationQuery, ObservationStore};
use crate::types::ToolId;

/// Manages observation capture and storage across a session
pub struct ObservationManager {
    session_id: String,
    observations: Vec<Observation>,
}

impl ObservationManager {
    /// Persist all observations to a SQLite database at `path`.
    ///
    /// The database and schema are initialized when absent. An existing,
    /// incompatible schema returns an error instead of being silently changed.
    pub fn persist(&self, path: &std::path::Path) -> anyhow::Result<()> {
        SqliteObservationStore::new(path).save(&self.observations)
    }

    /// Load observations for `session_id` in oldest-first replay order.
    ///
    /// `path` must already contain a compatible observation schema. This read
    /// never creates or migrates a database; an unknown session returns an
    /// empty manager after the schema is validated.
    pub fn load_from(session_id: &str, path: &std::path::Path) -> anyhow::Result<Self> {
        let observations = SqliteObservationStore::new(path).for_session(session_id)?;
        Ok(Self {
            session_id: session_id.to_string(),
            observations,
        })
    }

    /// Query observations by tool name across sessions, newest-first.
    ///
    /// A zero limit returns an empty collection without opening `path`.
    pub fn query_by_tool(
        path: &std::path::Path,
        tool_name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>> {
        SqliteObservationStore::new(path).by_tool(tool_name, limit)
    }

    /// Query most recent observations across all sessions, newest-first.
    ///
    /// Equal timestamps use reverse insertion order. A zero limit returns an
    /// empty collection without opening `path`.
    pub fn query_recent(path: &std::path::Path, limit: usize) -> anyhow::Result<Vec<Observation>> {
        SqliteObservationStore::new(path).recent(limit)
    }

    /// Replay observations from another session into the current session cache.
    pub fn replay_from_session(
        &mut self,
        path: &std::path::Path,
        source_session_id: &str,
    ) -> anyhow::Result<usize> {
        self.replay_from(&SqliteObservationStore::new(path), source_session_id)
    }

    /// Replay a source session through a query port into this runtime cache.
    ///
    /// Replay preserves the source session ID and timestamp. Repeating a replay
    /// is idempotent, while distinct observations sharing a timestamp are kept.
    /// The query must return the source session in oldest-first replay order and
    /// must validate its own backing schema before returning an unknown session
    /// as empty. The target manager may already contain captured observations;
    /// replay only appends source observations whose complete identity is new.
    ///
    /// ```no_run
    /// use looprs::{ObservationManager, ObservationQuery, SqliteObservationStore};
    ///
    /// let source = SqliteObservationStore::new("observations.db");
    /// let mut target = ObservationManager::new();
    /// let replayed = target.replay_from(&source, "source-session")?;
    /// assert_eq!(target.count(), replayed);
    /// // The same source rows are not appended twice.
    /// assert_eq!(target.replay_from(&source, "source-session")?, 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn replay_from(
        &mut self,
        query: &dyn ObservationQuery,
        source_session_id: &str,
    ) -> anyhow::Result<usize> {
        let source = query.for_session(source_session_id)?;
        let mut seen = self
            .observations
            .iter()
            .map(observation_identity)
            .collect::<HashSet<_>>();

        let mut replayed = 0usize;
        for obs in source {
            let key = observation_identity(&obs);
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

fn observation_identity(
    observation: &Observation,
) -> (
    String,
    String,
    String,
    String,
    Option<String>,
    u64,
    Option<String>,
) {
    (
        observation.session_id.clone(),
        observation.tool_name.clone(),
        observation.input.to_string(),
        observation.output.clone(),
        observation
            .tool_use_id
            .as_ref()
            .map(|id| id.as_str().to_string()),
        observation.timestamp,
        observation.context.clone(),
    )
}

impl Default for ObservationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(session_id: &str, output: &str, timestamp: u64) -> Observation {
        Observation {
            tool_name: "read".to_string(),
            input: serde_json::json!({"path": output}),
            output: output.to_string(),
            tool_use_id: None,
            timestamp,
            session_id: session_id.to_string(),
            context: None,
        }
    }

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
    fn repeated_persist_does_not_duplicate_observations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");

        let mut manager = ObservationManager::new();
        manager.capture(
            "read".to_string(),
            serde_json::json!({"file": "foo.rs"}),
            "contents".to_string(),
            None,
        );

        manager.persist(&path).unwrap();
        manager.persist(&path).unwrap();

        let loaded = ObservationManager::load_from(manager.session_id(), &path).unwrap();
        assert_eq!(loaded.count(), 1);
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

    #[test]
    fn query_recent_zero_limit_does_not_create_missing_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.db");

        assert!(
            ObservationManager::query_recent(&path, 0)
                .unwrap()
                .is_empty()
        );
        assert!(!path.exists());
    }

    #[test]
    fn query_recent_missing_database_returns_precise_error_without_creating_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.db");

        let error = ObservationManager::query_recent(&path, 1).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("observation database does not exist")
        );
        assert!(!path.exists());
    }

    #[test]
    fn query_recent_orders_deterministically_and_honors_one_row_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let mut manager = ObservationManager::new();
        manager.observations = vec![
            observation("first", "old", 9),
            observation("second", "newer-first", 10),
            observation("third", "newer-last", 10),
        ];
        manager.persist(&path).unwrap();

        let all = ObservationManager::query_recent(&path, 10).unwrap();
        let one = ObservationManager::query_recent(&path, 1).unwrap();

        assert_eq!(
            all.iter()
                .map(|item| item.output.as_str())
                .collect::<Vec<_>>(),
            vec!["newer-last", "newer-first", "old"]
        );
        assert_eq!(one[0].output, "newer-last");
    }

    #[test]
    fn query_recent_returns_cross_session_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let mut manager = ObservationManager::new();
        manager.observations = vec![
            observation("session-a", "a", 1),
            observation("session-b", "b", 2),
        ];
        manager.persist(&path).unwrap();

        let rows = ObservationManager::query_recent(&path, 10).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session_id, "session-b");
        assert_eq!(rows[1].session_id, "session-a");
    }

    #[test]
    fn query_recent_rejects_malformed_json_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let manager = ObservationManager::new();
        manager.persist(&path).unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO observations
                 (session_id, tool_name, input, output, timestamp)
                 VALUES ('broken', 'read', 'not-json', 'output', 1)",
                [],
            )
            .unwrap();

        let error = ObservationManager::query_recent(&path, 1).unwrap_err();

        assert!(error.to_string().contains("invalid observation input JSON"));
    }

    #[test]
    fn query_recent_rejects_incompatible_schema_precisely() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute("CREATE TABLE observations (session_id TEXT NOT NULL)", [])
            .unwrap();

        let error = ObservationManager::query_recent(&path, 1).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("incompatible observations schema")
        );
        assert!(error.to_string().contains("tool_name"));
    }

    #[test]
    fn replay_is_idempotent_and_unknown_sessions_are_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let mut source = ObservationManager::new();
        source.observations = vec![observation("source", "one", 10)];
        source.persist(&path).unwrap();
        let mut target = ObservationManager::new();

        assert_eq!(target.replay_from_session(&path, "source").unwrap(), 1);
        assert_eq!(target.replay_from_session(&path, "source").unwrap(), 0);
        assert_eq!(target.replay_from_session(&path, "unknown").unwrap(), 0);
        assert_eq!(target.count(), 1);
    }

    #[test]
    fn replay_preserves_distinct_observations_with_timestamp_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("obs.db");
        let mut source = ObservationManager::new();
        source.observations = vec![
            observation("source", "first", 10),
            observation("source", "second", 10),
        ];
        source.persist(&path).unwrap();
        let mut target = ObservationManager::new();

        assert_eq!(target.replay_from_session(&path, "source").unwrap(), 2);
        assert_eq!(target.count(), 2);
    }
}
