//! SystemMonitor — collects real-time CPU, memory, and process metrics.
//!
//! Kept free of GUI and async-executor dependencies.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use sysinfo::{Pid, System};

const ERROR_WINDOW_SECS: u64 = 60;
const CPU_LOW_THRESHOLD: f32 = 50.0;
const CPU_HIGH_THRESHOLD: f32 = 80.0;
const RESPONSE_LOW_MS: f64 = 10.0;
const RESPONSE_MID_MS: f64 = 50.0;
const RESPONSE_HIGH_MS: f64 = 200.0;

/// Snapshot of system-level performance metrics.
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// Average CPU usage across all cores (0–100).
    pub cpu_usage: f64,
    /// Used memory as a percentage of total (0–100).
    pub memory_usage: f64,
    /// Number of errors recorded in the last 60 seconds.
    pub error_rate: f64,
    /// Estimated P95 response time in milliseconds (heuristic).
    pub response_time_p95: f64,
}

/// Process-level metrics for the current process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub cpu_usage: f64,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
}

/// Collects real-time system metrics and tracks a rolling error window.
pub struct SystemMonitor {
    sys: System,
    error_count_window: VecDeque<(Instant, usize)>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            error_count_window: VecDeque::with_capacity(60),
        }
    }

    /// Refresh and return current system metrics.
    pub fn collect_metrics(&mut self) -> SystemMetrics {
        self.sys.refresh_cpu();
        self.sys.refresh_memory();

        let cpu_usage = self.sys.global_cpu_info().cpu_usage() as f64;

        let total = self.sys.total_memory() as f64;
        let used = self.sys.used_memory() as f64;
        let memory_usage = if total > 0.0 {
            (used / total) * 100.0
        } else {
            0.0
        };

        let error_rate = self.flush_error_window();
        let response_time_p95 = self.estimate_response_time();

        SystemMetrics {
            cpu_usage,
            memory_usage,
            error_rate,
            response_time_p95,
        }
    }

    /// Record an error occurrence for rate tracking.
    pub fn record_error(&mut self) {
        let now = Instant::now();
        self.error_count_window.push_back((now, 1));
        self.evict_old_entries(now);
    }

    /// Return current process CPU/memory info.
    pub fn process_info(&mut self) -> Option<ProcessInfo> {
        let pid = Pid::from_u32(std::process::id());
        self.sys.refresh_process(pid);
        self.sys.process(pid).map(|p| ProcessInfo {
            cpu_usage: p.cpu_usage() as f64,
            memory_bytes: p.memory(),
            virtual_memory_bytes: p.virtual_memory(),
        })
    }

    fn flush_error_window(&mut self) -> f64 {
        let now = Instant::now();
        self.evict_old_entries(now);
        self.error_count_window
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>() as f64
    }

    fn evict_old_entries(&mut self, now: Instant) {
        let cutoff = now - Duration::from_secs(ERROR_WINDOW_SECS);
        while let Some((ts, _)) = self.error_count_window.front() {
            if *ts < cutoff {
                self.error_count_window.pop_front();
            } else {
                break;
            }
        }
    }

    fn estimate_response_time(&self) -> f64 {
        let cpu = self.sys.global_cpu_info().cpu_usage();
        if cpu < CPU_LOW_THRESHOLD {
            RESPONSE_LOW_MS
        } else if cpu < CPU_HIGH_THRESHOLD {
            RESPONSE_MID_MS
        } else {
            RESPONSE_HIGH_MS
        }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_and_memory_in_range() {
        let mut monitor = SystemMonitor::new();
        let m = monitor.collect_metrics();
        assert!((0.0..=100.0).contains(&m.cpu_usage));
        assert!((0.0..=100.0).contains(&m.memory_usage));
    }

    #[test]
    fn error_rate_starts_at_zero() {
        let mut monitor = SystemMonitor::new();
        let m = monitor.collect_metrics();
        assert_eq!(m.error_rate, 0.0);
    }

    #[test]
    fn error_rate_counts_recent_errors() {
        let mut monitor = SystemMonitor::new();
        monitor.record_error();
        monitor.record_error();
        monitor.record_error();
        let m = monitor.collect_metrics();
        assert_eq!(m.error_rate, 3.0);
    }

    #[test]
    fn old_errors_are_evicted() {
        let mut monitor = SystemMonitor::new();

        // Inject a stale entry first so it sorts before the fresh one.
        let old = Instant::now() - Duration::from_secs(61);
        monitor.error_count_window.push_back((old, 1));

        monitor.record_error(); // recent — added after, so it's at the back

        let m = monitor.collect_metrics();
        assert_eq!(m.error_rate, 1.0);
    }

    #[test]
    fn process_info_returns_some() {
        let mut monitor = SystemMonitor::new();
        let info = monitor.process_info();
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.memory_bytes > 0);
    }
}
