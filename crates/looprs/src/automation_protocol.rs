//! Typed machine-readable event protocol and run cancellation controls.
//!
//! `LOOPRS_MACHINE_LOG=1` retains the legacy top-level `{kind,data}` JSONL
//! records. Set `LOOPRS_MACHINE_PROTOCOL=looprs-machine/v1` to opt into the
//! versioned envelope produced by [`next_envelope`]. Configuration is
//! process-global: a generated run ID remains stable for the process, v1
//! sequence numbers increase for every emitted envelope, and an invalid
//! explicit protocol disables machine output rather than selecting a fallback.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Enables the backward-compatible top-level `{kind,data}` JSONL format.
pub const MACHINE_LOG_ENV: &str = "LOOPRS_MACHINE_LOG";
/// Selects an explicit versioned machine protocol.
pub const MACHINE_PROTOCOL_ENV: &str = "LOOPRS_MACHINE_PROTOCOL";
/// Overrides the stable identifier attached to every event in one run.
pub const MACHINE_RUN_ID_ENV: &str = "LOOPRS_MACHINE_RUN_ID";
/// Stores an absolute Unix epoch deadline in milliseconds.
pub const MACHINE_DEADLINE_MS_ENV: &str = "LOOPRS_MACHINE_DEADLINE_MS";
/// Stores a path whose existence requests run cancellation.
pub const MACHINE_CANCEL_FILE_ENV: &str = "LOOPRS_MACHINE_CANCEL_FILE";
/// Wire identifier for the first versioned machine protocol.
pub const MACHINE_PROTOCOL_V1: &str = "looprs-machine/v1";

const CANCELLATION_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static GENERATED_RUN_ID: OnceLock<String> = OnceLock::new();

/// Supported machine event formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineProtocol {
    /// Historical top-level `{kind,data}` records selected by `--machine-log`.
    #[serde(skip)]
    Legacy,
    /// Versioned envelopes selected explicitly by `--machine-protocol`.
    #[serde(rename = "looprs-machine/v1")]
    V1,
}

impl MachineProtocol {
    /// Returns the stable wire identifier, if this is a versioned protocol.
    pub const fn wire_name(self) -> Option<&'static str> {
        match self {
            Self::Legacy => None,
            Self::V1 => Some(MACHINE_PROTOCOL_V1),
        }
    }
}

impl FromStr for MachineProtocol {
    type Err = UnsupportedMachineProtocol;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            MACHINE_PROTOCOL_V1 => Ok(Self::V1),
            other => Err(UnsupportedMachineProtocol(other.to_string())),
        }
    }
}

/// Error returned when a caller requests an unknown protocol version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMachineProtocol(String);

impl fmt::Display for UnsupportedMachineProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported machine protocol {:?}, expected {MACHINE_PROTOCOL_V1:?}",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedMachineProtocol {}

/// Typed machine event payload shared by legacy records and v1 envelopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineEvent {
    /// Stable event kind, such as `run.started` or `run.succeeded`.
    pub kind: String,
    /// Event-specific JSON payload.
    pub data: Value,
}

/// Versioned machine protocol envelope.
///
/// # Examples
///
/// ```
/// use looprs::automation_protocol::{MachineEnvelope, MachineEvent, MachineProtocol};
///
/// let envelope = MachineEnvelope {
///     protocol: MachineProtocol::V1,
///     run_id: "build-42".to_string(),
///     sequence: 1,
///     timestamp: "2026-01-01T00:00:00Z".to_string(),
///     event: MachineEvent {
///         kind: "run.started".to_string(),
///         data: serde_json::json!({"scriptable": true}),
///     },
/// };
/// assert_eq!(serde_json::to_value(envelope)?["seq"], 1);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineEnvelope {
    /// Protocol version for this record.
    pub protocol: MachineProtocol,
    /// Stable identity shared by every record in the run.
    pub run_id: String,
    /// Process-monotonic event sequence number.
    #[serde(rename = "seq")]
    pub sequence: u64,
    /// RFC 3339 UTC emission timestamp.
    #[serde(rename = "ts")]
    pub timestamp: String,
    /// Typed event and payload.
    pub event: MachineEvent,
}

/// A serialized machine record in either backward-compatible or v1 form.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MachineRecord {
    /// Legacy top-level event.
    Legacy(MachineEvent),
    /// Explicit v1 envelope.
    V1(MachineEnvelope),
}

/// Why an active machine run was cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    /// The configured absolute deadline was reached.
    DeadlineExceeded,
    /// The configured cancellation file exists.
    CancelRequested,
}

impl CancellationReason {
    /// Returns the stable reason label used in machine events.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::CancelRequested => "cancel_requested",
        }
    }
}

/// Validation error for relative run controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunControlError {
    /// Relative deadlines must be at least one second.
    ZeroTimeout,
    /// Cancellation paths must contain a non-whitespace path.
    BlankCancelPath,
}

impl fmt::Display for RunControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTimeout => formatter.write_str("run timeout must be greater than zero"),
            Self::BlankCancelPath => formatter.write_str("cancellation path cannot be blank"),
        }
    }
}

impl std::error::Error for RunControlError {}

/// Validated controls used to interrupt an in-flight machine run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunControls {
    deadline_millis: Option<u128>,
    cancel_file: Option<PathBuf>,
}

impl RunControls {
    /// Creates validated controls from a relative timeout and cancellation path.
    pub fn try_from_timeout(
        timeout_seconds: Option<u64>,
        cancel_file: Option<PathBuf>,
    ) -> Result<Self, RunControlError> {
        if timeout_seconds == Some(0) {
            return Err(RunControlError::ZeroTimeout);
        }
        if cancel_file
            .as_deref()
            .is_some_and(|path| path.as_os_str().to_string_lossy().trim().is_empty())
        {
            return Err(RunControlError::BlankCancelPath);
        }
        let deadline_millis = timeout_seconds.map(|seconds| {
            current_epoch_millis().saturating_add(u128::from(seconds).saturating_mul(1_000))
        });
        Ok(Self {
            deadline_millis,
            cancel_file,
        })
    }

    /// Reads absolute deadline and cancellation-file controls from the environment.
    ///
    /// Malformed deadlines and blank cancellation paths are ignored.
    pub fn from_env() -> Self {
        let deadline_millis = std::env::var(MACHINE_DEADLINE_MS_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<u128>().ok());
        let cancel_file = std::env::var(MACHINE_CANCEL_FILE_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            deadline_millis,
            cancel_file,
        }
    }

    /// Creates controls with an absolute Unix epoch deadline in milliseconds.
    pub fn with_deadline_millis(deadline_millis: u128) -> Self {
        Self {
            deadline_millis: Some(deadline_millis),
            cancel_file: None,
        }
    }

    /// Returns the current cancellation reason, if any.
    pub fn cancellation(&self) -> Option<CancellationReason> {
        self.cancellation_at(current_epoch_millis())
    }

    /// Returns the cancellation reason at a supplied Unix epoch millisecond.
    pub fn cancellation_at(&self, now_millis: u128) -> Option<CancellationReason> {
        if self.deadline_exceeded_at(now_millis) {
            return Some(CancellationReason::DeadlineExceeded);
        }
        if self.cancel_file_exists() {
            return Some(CancellationReason::CancelRequested);
        }
        None
    }

    /// Returns whether the deadline has elapsed at a supplied epoch millisecond.
    pub fn deadline_exceeded_at(&self, now_millis: u128) -> bool {
        self.deadline_millis
            .is_some_and(|deadline| now_millis >= deadline)
    }

    /// Returns whether the configured cancellation file currently exists.
    pub fn cancel_file_exists(&self) -> bool {
        self.cancel_file.as_deref().is_some_and(Path::exists)
    }

    /// Waits until the deadline expires or the cancellation file appears.
    ///
    /// If no controls are configured, this future remains pending.
    pub async fn cancelled(&self) -> CancellationReason {
        if self.deadline_millis.is_none() && self.cancel_file.is_none() {
            return std::future::pending().await;
        }
        loop {
            if let Some(reason) = self.cancellation() {
                return reason;
            }
            tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
        }
    }
}

/// Returns whether legacy or versioned machine output is enabled.
pub fn machine_logging_enabled() -> bool {
    selected_protocol().is_some()
}

/// Selects the configured protocol, validating explicit version strings.
///
/// An invalid explicit protocol disables output rather than silently falling
/// back to the legacy format.
pub fn selected_protocol() -> Option<MachineProtocol> {
    if let Ok(protocol) = std::env::var(MACHINE_PROTOCOL_ENV) {
        return protocol.parse().ok();
    }

    if std::env::var(MACHINE_LOG_ENV)
        .ok()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true"))
    {
        return Some(MachineProtocol::Legacy);
    }

    None
}

/// Returns an explicit run ID or a process-stable generated fallback.
pub fn run_id() -> String {
    if let Ok(explicit) = std::env::var(MACHINE_RUN_ID_ENV) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    GENERATED_RUN_ID
        .get_or_init(|| {
            let now_ms = current_epoch_millis();
            format!("run-{now_ms}-{}", std::process::id())
        })
        .clone()
}

/// Builds the next typed v1 envelope when v1 is explicitly selected.
pub fn next_envelope(kind: &str, data: Value) -> Option<MachineEnvelope> {
    if selected_protocol()? != MachineProtocol::V1 {
        return None;
    }
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;

    Some(MachineEnvelope {
        protocol: MachineProtocol::V1,
        run_id: run_id(),
        sequence,
        timestamp: chrono::Utc::now().to_rfc3339(),
        event: MachineEvent {
            kind: kind.to_string(),
            data,
        },
    })
}

/// Builds the next record in the selected legacy or versioned format.
pub fn next_record(kind: &str, data: Value) -> Option<MachineRecord> {
    match selected_protocol()? {
        MachineProtocol::Legacy => Some(MachineRecord::Legacy(MachineEvent {
            kind: kind.to_string(),
            data,
        })),
        MachineProtocol::V1 => next_envelope(kind, data).map(MachineRecord::V1),
    }
}

/// Returns true when the environment-configured absolute deadline has elapsed.
pub fn deadline_exceeded() -> bool {
    RunControls::from_env().deadline_exceeded_at(current_epoch_millis())
}

/// Returns true when the environment-configured cancellation file exists.
pub fn cancellation_requested() -> bool {
    RunControls::from_env().cancel_file_exists()
}

fn current_epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    const VARIABLES: [&str; 5] = [
        MACHINE_LOG_ENV,
        MACHINE_PROTOCOL_ENV,
        MACHINE_RUN_ID_ENV,
        MACHINE_DEADLINE_MS_ENV,
        MACHINE_CANCEL_FILE_ENV,
    ];

    pub(crate) struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        original: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        pub(crate) fn lock() -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let original = VARIABLES
                .into_iter()
                .map(|name| (name, std::env::var_os(name)))
                .collect();
            Self {
                _lock: lock,
                original,
            }
        }

        pub(crate) fn set(&self, name: &str, value: impl AsRef<OsStr>) {
            // SAFETY: all machine-protocol tests serialize environment access
            // through ENV_LOCK and this guard restores values on drop.
            unsafe { std::env::set_var(name, value) };
        }

        pub(crate) fn remove(&self, name: &str) {
            // SAFETY: all machine-protocol tests serialize environment access
            // through ENV_LOCK and this guard restores values on drop.
            unsafe { std::env::remove_var(name) };
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.original {
                // SAFETY: ENV_LOCK remains held until this restoration completes.
                unsafe {
                    if let Some(value) = value {
                        std::env::set_var(name, value);
                    } else {
                        std::env::remove_var(name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_log_env_selects_legacy_records() {
        let env = test_support::EnvGuard::lock();
        env.remove(MACHINE_PROTOCOL_ENV);
        env.set(MACHINE_LOG_ENV, "1");

        assert_eq!(selected_protocol(), Some(MachineProtocol::Legacy));
        let record = next_record("info", serde_json::json!({ "message": "ok" }))
            .expect("machine logging should be enabled");
        assert_eq!(
            serde_json::to_value(record).expect("record should serialize"),
            serde_json::json!({
                "kind": "info",
                "data": { "message": "ok" }
            })
        );
    }

    #[test]
    fn explicit_v1_envelope_contains_stable_fields() {
        let env = test_support::EnvGuard::lock();
        env.set(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        env.set(MACHINE_RUN_ID_ENV, "run-abc");

        let envelope = next_envelope("run.started", serde_json::json!({ "ok": true }))
            .expect("machine protocol should be enabled");
        assert_eq!(envelope.protocol, MachineProtocol::V1);
        assert_eq!(envelope.run_id, "run-abc");
        assert_eq!(envelope.event.kind, "run.started");
        assert_eq!(envelope.event.data["ok"], true);
        assert!(!envelope.timestamp.is_empty());
    }

    #[test]
    fn sequence_is_monotonic() {
        let env = test_support::EnvGuard::lock();
        env.set(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        let first = next_envelope("first", Value::Null).expect("first envelope");
        let second = next_envelope("second", Value::Null).expect("second envelope");
        assert_eq!(second.sequence, first.sequence + 1);
    }

    #[test]
    fn unsupported_or_blank_protocol_is_disabled() {
        let env = test_support::EnvGuard::lock();
        env.remove(MACHINE_LOG_ENV);
        for value in ["", "  ", "v2", "looprs-machine/v999"] {
            env.set(MACHINE_PROTOCOL_ENV, value);
            assert_eq!(selected_protocol(), None, "protocol {value:?}");
            assert!(next_record("event", Value::Null).is_none());
        }
    }

    #[test]
    fn false_like_machine_log_values_are_disabled() {
        let env = test_support::EnvGuard::lock();
        env.remove(MACHINE_PROTOCOL_ENV);
        for value in ["", "0", "false", "yes"] {
            env.set(MACHINE_LOG_ENV, value);
            assert!(!machine_logging_enabled(), "value {value:?}");
        }
    }

    #[test]
    fn generated_run_id_is_nonempty_and_stable() {
        let env = test_support::EnvGuard::lock();
        env.remove(MACHINE_RUN_ID_ENV);
        let first = run_id();
        assert!(!first.is_empty());
        assert_eq!(run_id(), first);
    }

    #[test]
    fn deadline_boundary_is_inclusive() {
        let controls = RunControls::with_deadline_millis(1_000);
        assert_eq!(controls.cancellation_at(999), None);
        assert_eq!(
            controls.cancellation_at(1_000),
            Some(CancellationReason::DeadlineExceeded)
        );
    }

    #[test]
    fn run_controls_reject_zero_timeout_and_blank_cancel_path() {
        assert!(RunControls::try_from_timeout(Some(0), None).is_err());
        assert!(RunControls::try_from_timeout(None, Some(PathBuf::from("  "))).is_err());
    }

    #[test]
    fn invalid_deadline_environment_is_ignored() {
        let env = test_support::EnvGuard::lock();
        for value in ["", "-1", "not-a-number"] {
            env.set(MACHINE_DEADLINE_MS_ENV, value);
            assert!(!deadline_exceeded(), "deadline {value:?}");
        }
    }

    #[test]
    fn cancellation_file_handles_existing_missing_and_blank_paths() {
        let env = test_support::EnvGuard::lock();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        env.set(MACHINE_CANCEL_FILE_ENV, tmp.path());
        assert!(cancellation_requested());
        env.set(
            MACHINE_CANCEL_FILE_ENV,
            tmp.path().with_extension("missing"),
        );
        assert!(!cancellation_requested());
        env.set(MACHINE_CANCEL_FILE_ENV, "  ");
        assert!(!cancellation_requested());
    }
}
