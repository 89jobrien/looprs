//! Builds orchestration metadata for turns initiated outside the REPL.

use std::collections::HashMap;

/// Returns metadata with `orchestration.mode` set to `mode`.
#[allow(dead_code)]
pub fn turn_metadata_with_mode(mode: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), mode.to_string());
    metadata
}

/// Returns orchestration metadata for a GUI-originated turn.
#[allow(dead_code)]
pub fn gui_turn_metadata() -> HashMap<String, String> {
    turn_metadata_with_mode("gui")
}
