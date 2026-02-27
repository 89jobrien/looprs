use anyhow::Result;
use async_trait::async_trait;
use maestro_companion::protocol::cli::CliMessage;
use maestro_companion::session::cli_process::CliLifecycleEvent;
use maestro_companion::session::process_manager::ProcessManager;
use maestro_test_ws::fixtures;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    NotStarted,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone)]
pub struct MockConfig {
    pub start_delay: Option<Duration>,
    pub stop_delay: Option<Duration>,
    pub auto_emit: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            start_delay: None,
            stop_delay: None,
            auto_emit: true,
        }
    }
}

#[derive(Debug)]
pub struct MockCliProcess {
    state: Arc<Mutex<ProcessState>>,
    pid: Arc<Mutex<Option<u32>>>,
    event_queue: Arc<Mutex<VecDeque<CliLifecycleEvent>>>,
    message_log: Arc<Mutex<Vec<CliMessage>>>,
    config: MockConfig,
    next_pid: Arc<Mutex<u32>>,
    start_time: Arc<Mutex<Option<Instant>>>,
}

impl MockCliProcess {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProcessState::NotStarted)),
            pid: Arc::new(Mutex::new(None)),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            message_log: Arc::new(Mutex::new(Vec::new())),
            config: MockConfig::default(),
            next_pid: Arc::new(Mutex::new(1000)),
            start_time: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_config(config: MockConfig) -> Self {
        let mut this = Self::new();
        this.config = config;
        this
    }

    pub fn inject_event(&self, event: CliLifecycleEvent) {
        self.event_queue.lock().unwrap().push_back(event);
    }

    pub fn message_count(&self) -> usize {
        self.message_log.lock().unwrap().len()
    }

    pub fn messages(&self) -> Vec<CliMessage> {
        self.message_log.lock().unwrap().clone()
    }

    pub fn clear_log(&self) {
        self.message_log.lock().unwrap().clear();
    }

    pub fn assert_received_message(&self, predicate: impl Fn(&CliMessage) -> bool) -> bool {
        self.message_log.lock().unwrap().iter().any(predicate)
    }

    pub fn find_message(&self, predicate: impl Fn(&CliMessage) -> bool) -> Option<CliMessage> {
        self.message_log
            .lock()
            .unwrap()
            .iter()
            .find(|msg| predicate(msg))
            .cloned()
    }

    pub fn filter_messages(&self, predicate: impl Fn(&CliMessage) -> bool) -> Vec<CliMessage> {
        self.message_log
            .lock()
            .unwrap()
            .iter()
            .filter(|msg| predicate(msg))
            .cloned()
            .collect()
    }

    pub fn state_name(&self) -> String {
        format!("{:?}", self.get_state())
    }

    pub fn pending_events_count(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }

    fn pop_event(&self) -> Option<CliLifecycleEvent> {
        self.event_queue.lock().unwrap().pop_front()
    }

    fn set_state(&self, state: ProcessState) {
        *self.state.lock().unwrap() = state;
    }

    fn get_state(&self) -> ProcessState {
        *self.state.lock().unwrap()
    }

    fn allocate_pid(&self) -> u32 {
        let mut next = self.next_pid.lock().unwrap();
        let pid = *next;
        *next = next.wrapping_add(1);
        drop(next);
        *self.pid.lock().unwrap() = Some(pid);
        pid
    }
}

impl Default for MockCliProcess {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProcessManager for MockCliProcess {
    async fn start(&mut self, _args: Vec<String>) -> Result<(u32, i64)> {
        let current_state = self.get_state();
        if !matches!(
            current_state,
            ProcessState::NotStarted | ProcessState::Stopped
        ) {
            return Err(anyhow::anyhow!(
                "Cannot start process in {:?} state",
                current_state
            ));
        }
        self.set_state(ProcessState::Starting);
        #[allow(clippy::disallowed_methods)]
        if let Some(delay) = self.config.start_delay {
            tokio::time::sleep(delay).await;
        }
        let pid = self.allocate_pid();
        let timestamp = fixtures::current_timestamp();
        *self.start_time.lock().unwrap() = Some(Instant::now());
        if let Some(event) = self.pop_event() {
            match event {
                CliLifecycleEvent::Started { .. } => {
                    self.set_state(ProcessState::Running);
                    Ok((pid, timestamp))
                }
                CliLifecycleEvent::Crashed { .. } => {
                    self.set_state(ProcessState::Failed);
                    *self.start_time.lock().unwrap() = None;
                    Err(anyhow::anyhow!("CLI process failed to start"))
                }
                _ => {
                    self.set_state(ProcessState::Running);
                    Ok((pid, timestamp))
                }
            }
        } else {
            self.set_state(ProcessState::Running);
            Ok((pid, timestamp))
        }
    }

    async fn stop(&mut self, _force: bool) -> Result<(Option<i32>, i64)> {
        let current_state = self.get_state();
        if !matches!(
            current_state,
            ProcessState::Running | ProcessState::Starting | ProcessState::Failed
        ) {
            return Err(anyhow::anyhow!(
                "Cannot stop process in {:?} state",
                current_state
            ));
        }
        self.set_state(ProcessState::Stopping);
        #[allow(clippy::disallowed_methods)]
        if let Some(delay) = self.config.stop_delay {
            tokio::time::sleep(delay).await;
        }
        let timestamp = fixtures::current_timestamp();
        let exit_code = if let Some(CliLifecycleEvent::Stopped { exit_code, .. }) = self.pop_event()
        {
            exit_code
        } else {
            Some(0)
        };
        self.set_state(ProcessState::Stopped);
        *self.pid.lock().unwrap() = None;
        *self.start_time.lock().unwrap() = None;
        Ok((exit_code, timestamp))
    }

    fn is_running(&self) -> bool {
        matches!(
            self.get_state(),
            ProcessState::Starting | ProcessState::Running
        )
    }

    fn pid(&self) -> Option<u32> {
        if self.is_running() {
            *self.pid.lock().unwrap()
        } else {
            None
        }
    }

    fn uptime_secs(&self) -> Option<u64> {
        if self.is_running() {
            self.start_time
                .lock()
                .unwrap()
                .as_ref()
                .map(|start| start.elapsed().as_secs())
        } else {
            None
        }
    }

    async fn restart(&mut self, args: Vec<String>) -> Result<(u32, i64)> {
        if self.is_running() {
            self.stop(false).await?;
        }
        self.start(args).await
    }

    fn try_wait(&mut self) -> Option<(Option<i32>, i64)> {
        let queue = self.event_queue.lock().unwrap();
        if queue.is_empty() {
            return None;
        }
        drop(queue);
        let event = self.pop_event()?;
        match event {
            CliLifecycleEvent::Stopped {
                exit_code,
                timestamp,
            } => {
                self.set_state(ProcessState::Stopped);
                *self.pid.lock().unwrap() = None;
                *self.start_time.lock().unwrap() = None;
                Some((exit_code, timestamp))
            }
            CliLifecycleEvent::Crashed {
                exit_code,
                timestamp,
                ..
            } => {
                self.set_state(ProcessState::Failed);
                *self.pid.lock().unwrap() = None;
                *self.start_time.lock().unwrap() = None;
                Some((exit_code, timestamp))
            }
            _ => None,
        }
    }

    async fn wait(&mut self) -> Option<(Option<i32>, i64)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_process_starts_and_stops() {
        let mut mock = MockCliProcess::new();
        let (pid, _ts) = mock.start(vec![]).await.expect("start failed");
        assert!(pid > 0);
        assert!(mock.is_running());
        let (exit_code, _ts) = mock.stop(false).await.expect("stop failed");
        assert_eq!(exit_code, Some(0));
        assert!(!mock.is_running());
    }

    #[tokio::test]
    async fn mock_process_respects_injected_events() {
        let mut proc = MockCliProcess::new();
        proc.inject_event(fixtures::sample_cli_started(5555));
        let (pid, _ts) = proc.start(vec![]).await.expect("start failed");
        assert!(pid > 0);
    }

    #[test]
    fn mock_process_captures_state() {
        let mock = MockCliProcess::new();
        assert!(!mock.is_running());
        assert_eq!(mock.message_count(), 0);
    }

    #[tokio::test]
    async fn mock_process_tracks_uptime() {
        let mut mock = MockCliProcess::new();
        mock.start(vec![]).await.expect("start failed");
        assert!(mock.is_running());
        assert!(mock.uptime_secs().is_some());
        mock.stop(false).await.expect("stop failed");
        assert!(mock.uptime_secs().is_none());
    }

    #[tokio::test]
    async fn mock_process_validates_start_state_transitions() {
        let mut mock = MockCliProcess::new();
        mock.start(vec![]).await.expect("first start failed");
        let result = mock.start(vec![]).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot start process"));
    }

    #[tokio::test]
    async fn mock_process_validates_stop_state_transitions() {
        let mut mock = MockCliProcess::new();
        let result = mock.stop(false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot stop process"));
    }

    #[tokio::test]
    async fn mock_process_crash_scenario() {
        let mut mock = MockCliProcess::new();
        mock.inject_event(fixtures::sample_cli_crashed(None, Some(1)));
        let result = mock.start(vec![]).await;
        assert!(result.is_err());
        assert!(!mock.is_running());
    }

    #[test]
    fn mock_process_message_filtering() {
        let mock = MockCliProcess::new();
        let _msg1 = fixtures::sample_assistant_message();
        let _msg2 = fixtures::sample_result_message();
        assert_eq!(mock.message_count(), 0);
        assert!(mock.find_message(|_| true).is_none());
        assert_eq!(mock.filter_messages(|_| true).len(), 0);
    }

    #[test]
    fn mock_process_state_debugging() {
        let mock = MockCliProcess::new();
        assert_eq!(mock.state_name(), "NotStarted");
        assert_eq!(mock.pending_events_count(), 0);
        mock.inject_event(fixtures::sample_cli_started(1234));
        assert_eq!(mock.pending_events_count(), 1);
    }

    #[tokio::test]
    async fn mock_process_handles_injected_stop_event() {
        let mut mock = MockCliProcess::new();
        mock.start(vec![]).await.expect("start failed");
        mock.inject_event(fixtures::sample_cli_stopped(Some(42)));
        let (exit_code, _ts) = mock.stop(false).await.expect("stop failed");
        assert_eq!(exit_code, Some(42));
    }

    #[tokio::test]
    async fn mock_process_cleanup_on_stop() {
        let mut mock = MockCliProcess::new();
        let (pid, _) = mock.start(vec![]).await.expect("start failed");
        assert!(pid > 0);
        assert!(mock.pid().is_some());
        mock.stop(false).await.expect("stop failed");
        assert!(mock.pid().is_none());
    }
}
