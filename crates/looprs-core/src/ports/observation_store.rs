//! ObservationStore port — abstraction over observation persistence.

use crate::observation::Observation;

/// Port: persist captured observations to a durable store.
///
/// Implementations decide the backend (SQLite, filesystem, etc.).
pub trait ObservationStore: Send {
    /// Save a batch of observations. Called at session end.
    fn save(&self, observations: &[Observation]) -> Result<(), anyhow::Error>;
}
