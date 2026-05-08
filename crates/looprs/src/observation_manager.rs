use anyhow::Result;
use serde_json::Value;
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
}
