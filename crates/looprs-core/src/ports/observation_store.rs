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
/// Results are newest-first for [`ObservationQuery::by_tool`] and
/// [`ObservationQuery::recent`], and oldest-first for
/// [`ObservationQuery::for_session`]. A zero limit returns an empty collection.
pub trait ObservationQuery: Send + Sync {
    /// Return all observations for one session in replay order.
    fn for_session(&self, session_id: &str) -> Result<Vec<Observation>, anyhow::Error>;

    /// Return the newest observations for a tool across all sessions.
    fn by_tool(&self, tool_name: &str, limit: usize) -> Result<Vec<Observation>, anyhow::Error>;

    /// Return the newest observations across all sessions.
    fn recent(&self, limit: usize) -> Result<Vec<Observation>, anyhow::Error>;
}
