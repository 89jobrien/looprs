use serde_json::Value;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MACHINE_LOG_ENV: &str = "LOOPRS_MACHINE_LOG";
pub const MACHINE_PROTOCOL_ENV: &str = "LOOPRS_MACHINE_PROTOCOL";
pub const MACHINE_RUN_ID_ENV: &str = "LOOPRS_MACHINE_RUN_ID";
pub const MACHINE_DEADLINE_MS_ENV: &str = "LOOPRS_MACHINE_DEADLINE_MS";
pub const MACHINE_CANCEL_FILE_ENV: &str = "LOOPRS_MACHINE_CANCEL_FILE";
pub const MACHINE_PROTOCOL_V1: &str = "looprs-machine/v1";

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static GENERATED_RUN_ID: OnceLock<String> = OnceLock::new();

pub fn machine_logging_enabled() -> bool {
    selected_protocol().is_some()
}

pub fn selected_protocol() -> Option<String> {
    if let Ok(protocol) = std::env::var(MACHINE_PROTOCOL_ENV) {
        let trimmed = protocol.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if matches!(
        std::env::var(MACHINE_LOG_ENV)
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1") | Some("true")
    ) {
        return Some(MACHINE_PROTOCOL_V1.to_string());
    }

    None
}

pub fn run_id() -> String {
    if let Ok(explicit) = std::env::var(MACHINE_RUN_ID_ENV) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    GENERATED_RUN_ID
        .get_or_init(|| {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            format!("run-{now_ms}-{}", std::process::id())
        })
        .clone()
}

pub fn next_envelope(kind: &str, data: Value) -> Option<Value> {
    let protocol = selected_protocol()?;
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;

    Some(serde_json::json!({
        "protocol": protocol,
        "run_id": run_id(),
        "seq": sequence,
        "ts": chrono::Utc::now().to_rfc3339(),
        "event": {
            "kind": kind,
            "data": data,
        }
    }))
}

pub fn deadline_exceeded() -> bool {
    let Ok(raw) = std::env::var(MACHINE_DEADLINE_MS_ENV) else {
        return false;
    };
    let Ok(deadline_ms) = raw.trim().parse::<u128>() else {
        return false;
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    now_ms >= deadline_ms
}

pub fn cancellation_requested() -> bool {
    let Ok(path) = std::env::var(MACHINE_CANCEL_FILE_ENV) else {
        return false;
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    std::path::Path::new(trimmed).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_log_env_defaults_to_v1_protocol() {
        // SAFETY: test-only environment mutation.
        unsafe {
            std::env::remove_var(MACHINE_PROTOCOL_ENV);
            std::env::set_var(MACHINE_LOG_ENV, "1");
        }

        assert_eq!(selected_protocol().as_deref(), Some(MACHINE_PROTOCOL_V1));
    }

    #[test]
    fn envelope_contains_protocol_run_id_and_event_payload() {
        // SAFETY: test-only environment mutation.
        unsafe {
            std::env::set_var(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
            std::env::set_var(MACHINE_RUN_ID_ENV, "run-abc");
        }

        let envelope = next_envelope("run.started", serde_json::json!({ "ok": true }))
            .expect("machine protocol should be enabled");
        assert_eq!(envelope["protocol"], MACHINE_PROTOCOL_V1);
        assert_eq!(envelope["run_id"], "run-abc");
        assert_eq!(envelope["event"]["kind"], "run.started");
        assert_eq!(envelope["event"]["data"]["ok"], true);
    }

    #[test]
    fn cancellation_requested_checks_file_existence() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // SAFETY: test-only environment mutation.
        unsafe {
            std::env::set_var(MACHINE_CANCEL_FILE_ENV, tmp.path());
        }
        assert!(cancellation_requested());
    }
}
