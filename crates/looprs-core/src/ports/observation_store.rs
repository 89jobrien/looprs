//! Focused ports for writing and querying observations.

use crate::observation::Observation;

/// Port for persisting captured observations to a durable store.
///
/// Implementations decide the backend (SQLite, filesystem, etc.).
///
/// # Example
///
/// ```
/// use looprs_core::{observation::Observation, ports::ObservationStore};
///
/// struct Sink;
/// impl ObservationStore for Sink {
///     fn save(&self, _observations: &[Observation]) -> anyhow::Result<()> {
///         Ok(())
///     }
/// }
///
/// let sink = Sink;
/// sink.save(&[])?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub trait ObservationStore: Send {
    /// Save a batch of observations. Called at session end.
    fn save(&self, observations: &[Observation]) -> Result<(), anyhow::Error>;
}

/// Port for reading observations without coupling runtime policy to a backend.
///
/// Implementations must return [`ObservationQuery::by_tool`] and
/// [`ObservationQuery::recent`] results newest-first. Equal timestamps retain a
/// deterministic backend-defined order. [`ObservationQuery::for_session`] is
/// oldest-first so its output can be replayed directly. Limited queries must
/// return an empty collection for a zero limit without opening or creating the
/// backing store.
///
/// ```
/// use anyhow::Result;
/// use looprs_core::{
///     observation::Observation,
///     ports::ObservationQuery,
/// };
///
/// struct MemoryQuery(Vec<Observation>);
///
/// impl ObservationQuery for MemoryQuery {
///     fn for_session(&self, session_id: &str) -> Result<Vec<Observation>> {
///         let mut rows = self.0.iter()
///             .filter(|row| row.session_id == session_id)
///             .cloned()
///             .collect::<Vec<_>>();
///         rows.sort_by_key(|row| row.timestamp);
///         Ok(rows)
///     }
///
///     fn by_tool(&self, tool_name: &str, limit: usize) -> Result<Vec<Observation>> {
///         let mut rows = self.0.iter()
///             .filter(|row| row.tool_name == tool_name)
///             .cloned()
///             .collect::<Vec<_>>();
///         rows.sort_by_key(|row| std::cmp::Reverse(row.timestamp));
///         rows.truncate(limit);
///         Ok(rows)
///     }
///
///     fn recent(&self, limit: usize) -> Result<Vec<Observation>> {
///         let mut rows = self.0.clone();
///         rows.sort_by_key(|row| std::cmp::Reverse(row.timestamp));
///         rows.truncate(limit);
///         Ok(rows)
///     }
/// }
///
/// let observation = |timestamp| Observation {
///     tool_name: "read".into(),
///     input: serde_json::json!({}),
///     output: String::new(),
///     tool_use_id: None,
///     timestamp,
///     session_id: "session".into(),
///     context: None,
/// };
/// let query = MemoryQuery(vec![observation(1), observation(2)]);
/// assert_eq!(query.recent(1)?[0].timestamp, 2);
/// assert_eq!(query.for_session("session")?[0].timestamp, 1);
/// assert!(query.by_tool("read", 0)?.is_empty());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub trait ObservationQuery: Send + Sync {
    /// Return all observations for one session in oldest-first replay order.
    ///
    /// Unlike the limited queries, this method must validate and read the
    /// backing store even when the session is unknown; an unknown session
    /// returns an empty collection only after a successful query.
    fn for_session(&self, session_id: &str) -> Result<Vec<Observation>, anyhow::Error>;

    /// Return at most `limit` observations for a tool, newest-first.
    ///
    /// A zero limit returns immediately without accessing the backing store.
    fn by_tool(&self, tool_name: &str, limit: usize) -> Result<Vec<Observation>, anyhow::Error>;

    /// Return at most `limit` observations across all sessions, newest-first.
    ///
    /// A zero limit returns immediately without accessing the backing store.
    fn recent(&self, limit: usize) -> Result<Vec<Observation>, anyhow::Error>;
}
