use anyhow::Result;
use serde_json::Value;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::observation::Observation;

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
    pub fn capture(&mut self, tool_name: String, input: Value, output: String) {
        let obs = Observation::new(tool_name, input, output, self.session_id.clone());
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

    /// Save all observations to bd as issues
    pub fn save_to_bd(&self) -> Result<()> {
        if self.observations.is_empty() {
            return Ok(());
        }

        // Check if bd is available
        let bd_check = Command::new("bd").args(["--version"]).output();

        if bd_check.is_err() {
            // bd not installed, silently skip
            return Ok(());
        }

        // Try to create bd issues for each observation
        for obs in &self.observations {
            let title = obs.to_bd_title();
            let description = obs.to_bd_description();

            let output = Command::new("bd")
                .args([
                    "create",
                    &title,
                    "--description",
                    &description,
                    "--tags",
                    "observation,automated",
                ])
                .output();

            // Log but don't fail if individual observation save fails
            match output {
                Ok(output) if output.status.success() => {
                    // Saved successfully
                }
                Ok(output) => {
                    crate::ui::warn(format!(
                        "Warning: Failed to save observation: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                Err(e) => {
                    crate::ui::warn(format!("Warning: Error saving observation to bd: {e}"));
                }
            }
        }

        Ok(())
    }

    /// Clear all observations (usually called after saving to bd)
    pub fn clear(&mut self) {
        self.observations.clear();
    }
}

impl Default for ObservationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Load recent observations from bd
pub fn load_recent_observations(limit: usize) -> Option<Vec<String>> {
    // Check if bd is available
    let bd_check = Command::new("bd").args(["--version"]).output();

    if bd_check.is_err() {
        return None;
    }

    // Query bd for recent observations
    let output = Command::new("bd")
        .args([
            "list",
            "--tag",
            "observation",
            "--limit",
            &limit.to_string(),
            "--json",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut summaries = Vec::new();

    for line in output_str.lines() {
        if line.is_empty() {
            continue;
        }

        if let Ok(issue) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(title) = issue.get("title").and_then(|t| t.as_str()) {
                summaries.push(title.to_string());
            }
        }
    }

    if summaries.is_empty() {
        None
    } else {
        Some(summaries)
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
        );

        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.observations()[0].tool_name, "bash");
    }

    #[test]
    fn test_multiple_observations() {
        let mut mgr = ObservationManager::new();
        mgr.capture(
            "bash".to_string(),
            serde_json::json!({}),
            "out1".to_string(),
        );
        mgr.capture(
            "grep".to_string(),
            serde_json::json!({}),
            "out2".to_string(),
        );

        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn test_observation_manager_clear() {
        let mut mgr = ObservationManager::new();
        mgr.capture("bash".to_string(), serde_json::json!({}), "out".to_string());
        assert_eq!(mgr.count(), 1);

        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }
}
