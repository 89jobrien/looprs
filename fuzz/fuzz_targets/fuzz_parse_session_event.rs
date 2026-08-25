#![no_main]
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct FuzzLogLine {
    ts: String,
    session_id: String,
    #[serde(flatten)]
    event: serde_json::Value,
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Session log lines are JSONL with metadata + flattened event payload.
        // Deserialization of arbitrary bytes must never panic.
        for line in s.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(line) = serde_json::from_str::<FuzzLogLine>(line) {
                // Invariant: parsed JSONL line round-trips through serde.
                let json = serde_json::to_string(&line)
                    .expect("parsed session-log line must re-serialize");
                let back: FuzzLogLine = serde_json::from_str(&json)
                    .expect("re-serialized session-log line must re-parse");
                assert_eq!(
                    serde_json::to_value(&back).expect("value conversion should succeed"),
                    serde_json::to_value(&line).expect("value conversion should succeed"),
                    "session-log line must survive a JSON round trip unchanged"
                );

                // Invariant: flattened event object should contain the event tag.
                if let Some(obj) = back.event.as_object() {
                    let _ = obj.get("event");
                }
            }
        }
    }
});
