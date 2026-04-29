use std::collections::HashMap;

#[allow(dead_code)]
pub fn turn_metadata_with_mode(mode: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), mode.to_string());
    metadata
}

#[allow(dead_code)]
pub fn gui_turn_metadata() -> HashMap<String, String> {
    turn_metadata_with_mode("gui")
}
