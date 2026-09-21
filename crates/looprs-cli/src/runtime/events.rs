//! Builds orchestration metadata attached to CLI and GUI turns.

use std::collections::HashMap;

/// Builds turn metadata identifying the selected orchestration mode.
#[allow(dead_code)]
pub fn turn_metadata_with_mode(mode: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), mode.to_string());
    metadata
}

/// Builds turn metadata for GUI-originated orchestration.
#[allow(dead_code)]
pub fn gui_turn_metadata() -> HashMap<String, String> {
    turn_metadata_with_mode("gui")
}
