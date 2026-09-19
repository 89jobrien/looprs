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
use std::io;
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
static SYSTEM_ENVIRONMENT: SystemEnvironment = SystemEnvironment;
static SYSTEM_CLOCK: SystemClock = SystemClock;
static SYSTEM_CANCELLATION_FILES: SystemCancellationFiles = SystemCancellationFiles;
static PROCESS_IDENTITY: ProcessRunIdentity = ProcessRunIdentity;
static PROCESS_SEQUENCE: ProcessEventSequence = ProcessEventSequence;

/// Reads machine-protocol configuration without coupling protocol logic to process globals.
pub trait EnvironmentPort {
    /// Returns one environment value when present and valid Unicode.
    fn var(&self, name: &str) -> Option<String>;
}

/// Supplies wall-clock values used by run controls and v1 envelopes.
pub trait ClockPort {
    /// Returns Unix epoch time in milliseconds.
    fn epoch_millis(&self) -> u128;
    /// Returns the current UTC timestamp in RFC 3339 form.
    fn rfc3339_utc(&self) -> String;
}

/// Checks whether a cancellation marker exists.
pub trait CancellationFilePort {
    /// Returns whether `path` currently exists.
    fn exists(&self, path: &Path) -> bool;
}

/// Resolves the stable identity shared by events in one run.
pub trait RunIdentityPort {
    /// Returns a run identifier using the supplied environment and clock services.
    fn run_id(&self, environment: &dyn EnvironmentPort, clock: &dyn ClockPort) -> String;
}

/// Supplies monotonically increasing event sequence numbers.
pub trait EventSequencePort {
    /// Returns the next sequence number.
    fn next_sequence(&self) -> u64;
}

/// Receives a machine record after protocol selection and envelope construction.
pub trait EventSinkPort {
    /// Sink-specific error.
    type Error;

    /// Emits one selected machine record.
    fn emit(&mut self, record: &MachineRecord) -> Result<(), Self::Error>;
}

/// Process environment adapter used by the compatibility API.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnvironment;

impl EnvironmentPort for SystemEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// System clock adapter used by the compatibility API.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl ClockPort for SystemClock {
    fn epoch_millis(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }

    fn rfc3339_utc(&self) -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

/// Host filesystem adapter used for cancellation markers.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCancellationFiles;

impl CancellationFilePort for SystemCancellationFiles {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// Process-stable run identity adapter used by the compatibility API.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessRunIdentity;

impl RunIdentityPort for ProcessRunIdentity {
    fn run_id(&self, environment: &dyn EnvironmentPort, clock: &dyn ClockPort) -> String {
        if let Some(explicit) = environment.var(MACHINE_RUN_ID_ENV) {
            let trimmed = explicit.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        GENERATED_RUN_ID
            .get_or_init(|| format!("run-{}-{}", clock.epoch_millis(), std::process::id()))
            .clone()
    }
}

/// Process-monotonic sequencing adapter used by the compatibility API.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEventSequence;

impl EventSequencePort for ProcessEventSequence {
    fn next_sequence(&self) -> u64 {
        EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1
    }
}

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

/// Injectable machine-protocol service.
///
/// The service owns no process-global state. Callers choose focused adapters for
/// environment, time, identity, sequencing, and event delivery.
#[derive(Clone, Copy)]
pub struct AutomationProtocol<'a> {
    environment: &'a dyn EnvironmentPort,
    clock: &'a dyn ClockPort,
    identity: &'a dyn RunIdentityPort,
    sequence: &'a dyn EventSequencePort,
}

impl fmt::Debug for AutomationProtocol<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AutomationProtocol { .. }")
    }
}

impl<'a> AutomationProtocol<'a> {
    /// Creates a protocol service from focused ports.
    pub const fn new(
        environment: &'a dyn EnvironmentPort,
        clock: &'a dyn ClockPort,
        identity: &'a dyn RunIdentityPort,
        sequence: &'a dyn EventSequencePort,
    ) -> Self {
        Self {
            environment,
            clock,
            identity,
            sequence,
        }
    }

    /// Creates the process-backed service used by the legacy free functions.
    pub fn system() -> AutomationProtocol<'static> {
        AutomationProtocol::new(
            &SYSTEM_ENVIRONMENT,
            &SYSTEM_CLOCK,
            &PROCESS_IDENTITY,
            &PROCESS_SEQUENCE,
        )
    }

    /// Selects the configured protocol, validating explicit version strings.
    pub fn selected_protocol(&self) -> Option<MachineProtocol> {
        if let Some(protocol) = self.environment.var(MACHINE_PROTOCOL_ENV) {
            return protocol.parse().ok();
        }

        self.environment
            .var(MACHINE_LOG_ENV)
            .filter(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true"))
            .map(|_| MachineProtocol::Legacy)
    }

    /// Returns the stable identity for this service's run.
    pub fn run_id(&self) -> String {
        self.identity.run_id(self.environment, self.clock)
    }

    /// Builds the next typed v1 envelope when v1 is explicitly selected.
    pub fn next_envelope(&self, kind: &str, data: Value) -> Option<MachineEnvelope> {
        if self.selected_protocol()? != MachineProtocol::V1 {
            return None;
        }

        Some(MachineEnvelope {
            protocol: MachineProtocol::V1,
            run_id: self.run_id(),
            sequence: self.sequence.next_sequence(),
            timestamp: self.clock.rfc3339_utc(),
            event: MachineEvent {
                kind: kind.to_string(),
                data,
            },
        })
    }

    /// Builds the next record in the selected legacy or versioned format.
    pub fn next_record(&self, kind: &str, data: Value) -> Option<MachineRecord> {
        match self.selected_protocol()? {
            MachineProtocol::Legacy => Some(MachineRecord::Legacy(MachineEvent {
                kind: kind.to_string(),
                data,
            })),
            MachineProtocol::V1 => self.next_envelope(kind, data).map(MachineRecord::V1),
        }
    }

    /// Builds and emits one record through the supplied sink.
    pub fn emit<S: EventSinkPort>(
        &self,
        sink: &mut S,
        kind: &str,
        data: Value,
    ) -> Result<Option<MachineRecord>, S::Error> {
        let Some(record) = self.next_record(kind, data) else {
            return Ok(None);
        };
        sink.emit(&record)?;
        Ok(Some(record))
    }
}

/// JSONL event sink for machine-readable writers.
#[derive(Debug)]
pub struct JsonLineEventSink<W> {
    writer: W,
}

impl<W> JsonLineEventSink<W> {
    /// Wraps a writer as a machine event sink.
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Returns the wrapped writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: io::Write> EventSinkPort for JsonLineEventSink<W> {
    type Error = io::Error;

    fn emit(&mut self, record: &MachineRecord) -> Result<(), Self::Error> {
        serde_json::to_writer(&mut self.writer, record)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }
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
        Self::try_from_timeout_with_clock(timeout_seconds, cancel_file, &SYSTEM_CLOCK)
    }

    /// Creates validated controls using an injected clock.
    pub fn try_from_timeout_with_clock(
        timeout_seconds: Option<u64>,
        cancel_file: Option<PathBuf>,
        clock: &dyn ClockPort,
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
            clock
                .epoch_millis()
                .saturating_add(u128::from(seconds).saturating_mul(1_000))
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
        Self::from_environment(&SYSTEM_ENVIRONMENT)
    }

    /// Reads run controls through an injected environment service.
    pub fn from_environment(environment: &dyn EnvironmentPort) -> Self {
        let deadline_millis = environment
            .var(MACHINE_DEADLINE_MS_ENV)
            .and_then(|value| value.trim().parse::<u128>().ok());
        let cancel_file = environment
            .var(MACHINE_CANCEL_FILE_ENV)
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
        self.cancellation_with(&SYSTEM_CLOCK, &SYSTEM_CANCELLATION_FILES)
    }

    /// Returns the current cancellation reason through injected clock and filesystem ports.
    pub fn cancellation_with(
        &self,
        clock: &dyn ClockPort,
        cancellation_files: &dyn CancellationFilePort,
    ) -> Option<CancellationReason> {
        self.cancellation_at_with(clock.epoch_millis(), cancellation_files)
    }

    /// Returns the cancellation reason at a supplied Unix epoch millisecond.
    pub fn cancellation_at(&self, now_millis: u128) -> Option<CancellationReason> {
        self.cancellation_at_with(now_millis, &SYSTEM_CANCELLATION_FILES)
    }

    /// Returns the cancellation reason at a supplied time through an injected filesystem port.
    pub fn cancellation_at_with(
        &self,
        now_millis: u128,
        cancellation_files: &dyn CancellationFilePort,
    ) -> Option<CancellationReason> {
        if self.deadline_exceeded_at(now_millis) {
            return Some(CancellationReason::DeadlineExceeded);
        }
        if self.cancel_file_exists_with(cancellation_files) {
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
        self.cancel_file_exists_with(&SYSTEM_CANCELLATION_FILES)
    }

    /// Returns whether the configured marker exists through an injected filesystem port.
    pub fn cancel_file_exists_with(&self, cancellation_files: &dyn CancellationFilePort) -> bool {
        self.cancel_file
            .as_deref()
            .is_some_and(|path| cancellation_files.exists(path))
    }

    /// Waits until the deadline expires or the cancellation file appears.
    ///
    /// If no controls are configured, this future remains pending.
    pub async fn cancelled(&self) -> CancellationReason {
        self.cancelled_with(&SYSTEM_CLOCK, &SYSTEM_CANCELLATION_FILES)
            .await
    }

    /// Waits for cancellation through injected clock and filesystem ports.
    pub async fn cancelled_with(
        &self,
        clock: &dyn ClockPort,
        cancellation_files: &dyn CancellationFilePort,
    ) -> CancellationReason {
        if self.deadline_millis.is_none() && self.cancel_file.is_none() {
            return std::future::pending().await;
        }
        loop {
            if let Some(reason) = self.cancellation_with(clock, cancellation_files) {
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
    AutomationProtocol::system().selected_protocol()
}

/// Returns an explicit run ID or a process-stable generated fallback.
pub fn run_id() -> String {
    AutomationProtocol::system().run_id()
}

/// Builds the next typed v1 envelope when v1 is explicitly selected.
pub fn next_envelope(kind: &str, data: Value) -> Option<MachineEnvelope> {
    AutomationProtocol::system().next_envelope(kind, data)
}

/// Builds the next record in the selected legacy or versioned format.
pub fn next_record(kind: &str, data: Value) -> Option<MachineRecord> {
    AutomationProtocol::system().next_record(kind, data)
}

/// Returns true when the environment-configured absolute deadline has elapsed.
pub fn deadline_exceeded() -> bool {
    RunControls::from_env().deadline_exceeded_at(SYSTEM_CLOCK.epoch_millis())
}

/// Returns true when the environment-configured cancellation file exists.
pub fn cancellation_requested() -> bool {
    RunControls::from_env().cancel_file_exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::{HashMap, HashSet};

    #[derive(Default)]
    struct FakeEnvironment(HashMap<String, String>);

    impl FakeEnvironment {
        fn with(mut self, name: &str, value: &str) -> Self {
            self.0.insert(name.to_string(), value.to_string());
            self
        }
    }

    impl EnvironmentPort for FakeEnvironment {
        fn var(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    struct FixedClock;

    impl ClockPort for FixedClock {
        fn epoch_millis(&self) -> u128 {
            1_000
        }

        fn rfc3339_utc(&self) -> String {
            "2026-01-01T00:00:00+00:00".to_string()
        }
    }

    struct FixedIdentity;

    impl RunIdentityPort for FixedIdentity {
        fn run_id(&self, _environment: &dyn EnvironmentPort, _clock: &dyn ClockPort) -> String {
            "run-fixed".to_string()
        }
    }

    #[derive(Default)]
    struct LocalSequence(Cell<u64>);

    impl EventSequencePort for LocalSequence {
        fn next_sequence(&self) -> u64 {
            let next = self.0.get() + 1;
            self.0.set(next);
            next
        }
    }

    #[derive(Default)]
    struct FakeCancellationFiles(HashSet<PathBuf>);

    impl CancellationFilePort for FakeCancellationFiles {
        fn exists(&self, path: &Path) -> bool {
            self.0.contains(path)
        }
    }

    #[derive(Default)]
    struct RecordingSink(Vec<MachineRecord>);

    impl EventSinkPort for RecordingSink {
        type Error = std::convert::Infallible;

        fn emit(&mut self, record: &MachineRecord) -> Result<(), Self::Error> {
            self.0.push(record.clone());
            Ok(())
        }
    }

    fn protocol<'a>(
        environment: &'a FakeEnvironment,
        sequence: &'a LocalSequence,
    ) -> AutomationProtocol<'a> {
        AutomationProtocol::new(environment, &FixedClock, &FixedIdentity, sequence)
    }

    #[test]
    fn injected_environment_preserves_legacy_wire_record() {
        let environment = FakeEnvironment::default().with(MACHINE_LOG_ENV, "TRUE");
        let sequence = LocalSequence::default();
        let record = protocol(&environment, &sequence)
            .next_record("info", serde_json::json!({ "message": "ok" }))
            .expect("legacy logging should be enabled");
        assert_eq!(
            serde_json::to_value(record).expect("record should serialize"),
            serde_json::json!({
                "kind": "info",
                "data": { "message": "ok" }
            })
        );
    }

    #[test]
    fn injected_services_preserve_v1_wire_envelope() {
        let environment =
            FakeEnvironment::default().with(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        let sequence = LocalSequence::default();
        let envelope = protocol(&environment, &sequence)
            .next_envelope("run.started", serde_json::json!({ "ok": true }))
            .expect("machine protocol should be enabled");
        assert_eq!(
            serde_json::to_value(envelope).expect("envelope should serialize"),
            serde_json::json!({
                "protocol": "looprs-machine/v1",
                "run_id": "run-fixed",
                "seq": 1,
                "ts": "2026-01-01T00:00:00+00:00",
                "event": {"kind": "run.started", "data": {"ok": true}}
            })
        );
    }

    #[test]
    fn event_sink_receives_selected_records_in_sequence() {
        let environment =
            FakeEnvironment::default().with(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        let sequence = LocalSequence::default();
        let service = protocol(&environment, &sequence);
        let mut sink = RecordingSink::default();
        service.emit(&mut sink, "first", Value::Null).unwrap();
        service.emit(&mut sink, "second", Value::Null).unwrap();

        let sequences = sink
            .0
            .iter()
            .map(|record| match record {
                MachineRecord::V1(envelope) => envelope.sequence,
                MachineRecord::Legacy(_) => 0,
            })
            .collect::<Vec<_>>();
        assert_eq!(sequences, [1, 2]);
    }

    #[test]
    fn unsupported_or_blank_protocol_is_disabled() {
        for value in ["", "  ", "v2", "looprs-machine/v999"] {
            let environment = FakeEnvironment::default().with(MACHINE_PROTOCOL_ENV, value);
            let sequence = LocalSequence::default();
            let service = protocol(&environment, &sequence);
            assert_eq!(service.selected_protocol(), None, "protocol {value:?}");
            assert!(service.next_record("event", Value::Null).is_none());
        }
    }

    #[test]
    fn false_like_machine_log_values_are_disabled() {
        for value in ["", "0", "false", "yes"] {
            let environment = FakeEnvironment::default().with(MACHINE_LOG_ENV, value);
            let sequence = LocalSequence::default();
            assert!(
                protocol(&environment, &sequence)
                    .selected_protocol()
                    .is_none(),
                "value {value:?}"
            );
        }
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
        assert!(RunControls::try_from_timeout_with_clock(Some(0), None, &FixedClock).is_err());
        assert!(
            RunControls::try_from_timeout_with_clock(None, Some(PathBuf::from("  ")), &FixedClock)
                .is_err()
        );
    }

    #[test]
    fn injected_environment_ignores_invalid_deadlines() {
        for value in ["", "-1", "not-a-number"] {
            let environment = FakeEnvironment::default().with(MACHINE_DEADLINE_MS_ENV, value);
            let controls = RunControls::from_environment(&environment);
            assert!(!controls.deadline_exceeded_at(1_000), "deadline {value:?}");
        }
    }

    #[test]
    fn injected_clock_and_filesystem_control_cancellation_deterministically() {
        let marker = PathBuf::from("cancel.marker");
        let controls =
            RunControls::try_from_timeout_with_clock(Some(1), Some(marker.clone()), &FixedClock)
                .unwrap();
        let mut files = FakeCancellationFiles::default();

        assert_eq!(controls.cancellation_at_with(1_999, &files), None);
        files.0.insert(marker);
        assert_eq!(
            controls.cancellation_at_with(1_999, &files),
            Some(CancellationReason::CancelRequested)
        );
        assert_eq!(
            controls.cancellation_at_with(2_000, &FakeCancellationFiles::default()),
            Some(CancellationReason::DeadlineExceeded)
        );
    }

    #[test]
    fn json_line_sink_emits_one_compatible_line() {
        let environment =
            FakeEnvironment::default().with(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        let sequence = LocalSequence::default();
        let mut sink = JsonLineEventSink::new(Vec::new());

        protocol(&environment, &sequence)
            .emit(&mut sink, "run.started", serde_json::json!({"ok": true}))
            .unwrap();

        let output = sink.into_inner();
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["event"]["kind"], "run.started");
    }
}
