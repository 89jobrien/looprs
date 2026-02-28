use super::context_engine::SystemMetrics;
use sysinfo::{Pid, System};

/// System monitor collects real-time system metrics
pub struct SystemMonitor {
    sys: System,
    error_count_window: std::collections::VecDeque<(std::time::Instant, usize)>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            sys: System::new_all(),
            error_count_window: std::collections::VecDeque::with_capacity(60),
        }
    }

    /// Collect current system metrics
    pub async fn collect_metrics(&mut self) -> SystemMetrics {
        // Refresh system info
        self.sys.refresh_cpu();
        self.sys.refresh_memory();

        // CPU usage (average across all cores)
        let cpu_usage = self.sys.global_cpu_info().cpu_usage() as f64;

        // Memory usage percentage
        let total_memory = self.sys.total_memory() as f64;
        let used_memory = self.sys.used_memory() as f64;
        let memory_usage = if total_memory > 0.0 {
            (used_memory / total_memory) * 100.0
        } else {
            0.0
        };

        // Error rate (errors per minute)
        let error_rate = self.calculate_error_rate().await;

        // Response time P95 (placeholder - would need actual request tracking)
        let response_time_p95 = self.estimate_response_time();

        SystemMetrics {
            cpu_usage,
            memory_usage,
            error_rate,
            response_time_p95,
        }
    }

    /// Track an error occurrence
    pub fn record_error(&mut self) {
        let now = std::time::Instant::now();
        self.error_count_window.push_back((now, 1));

        // Clean old entries (older than 1 minute)
        let cutoff = now - std::time::Duration::from_secs(60);
        while let Some((timestamp, _)) = self.error_count_window.front() {
            if *timestamp < cutoff {
                self.error_count_window.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate error rate (errors per minute)
    async fn calculate_error_rate(&mut self) -> f64 {
        // Clean old entries
        let now = std::time::Instant::now();
        let cutoff = now - std::time::Duration::from_secs(60);
        self.error_count_window
            .retain(|(timestamp, _)| *timestamp >= cutoff);

        // Count total errors in the window
        let total_errors: usize = self.error_count_window.iter().map(|(_, count)| count).sum();

        total_errors as f64
    }

    /// Estimate response time (placeholder - would need actual request tracking)
    fn estimate_response_time(&self) -> f64 {
        // This is a placeholder. In a real system, you would track actual request latencies
        // and calculate the 95th percentile.

        // For now, estimate based on CPU usage:
        // - Low CPU (<50%): ~10ms
        // - Medium CPU (50-80%): ~50ms
        // - High CPU (>80%): ~200ms
        let cpu = self.sys.global_cpu_info().cpu_usage();
        if cpu < 50.0 {
            10.0
        } else if cpu < 80.0 {
            50.0
        } else {
            200.0
        }
    }

    /// Get current process info
    pub fn get_process_info(&mut self) -> Option<ProcessInfo> {
        let pid = Pid::from_u32(std::process::id());
        self.sys.refresh_process(pid);

        self.sys.process(pid).map(|process| ProcessInfo {
            cpu_usage: process.cpu_usage() as f64,
            memory_bytes: process.memory(),
            virtual_memory_bytes: process.virtual_memory(),
        })
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-specific metrics
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub cpu_usage: f64,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitor_creation() {
        let monitor = SystemMonitor::new();
        assert!(monitor.error_count_window.is_empty());
    }

    #[tokio::test]
    async fn test_collect_metrics() {
        let mut monitor = SystemMonitor::new();
        let metrics = monitor.collect_metrics().await;

        // CPU should be between 0-100
        assert!(metrics.cpu_usage >= 0.0);
        assert!(metrics.cpu_usage <= 100.0);

        // Memory should be between 0-100
        assert!(metrics.memory_usage >= 0.0);
        assert!(metrics.memory_usage <= 100.0);

        // Error rate should be 0 initially
        assert_eq!(metrics.error_rate, 0.0);
    }

    #[tokio::test]
    async fn test_error_tracking() {
        let mut monitor = SystemMonitor::new();

        // Record some errors
        monitor.record_error();
        monitor.record_error();
        monitor.record_error();

        let metrics = monitor.collect_metrics().await;
        assert_eq!(metrics.error_rate, 3.0);
    }

    #[tokio::test]
    async fn test_error_window_expiry() {
        let mut monitor = SystemMonitor::new();

        // Record an error
        monitor.record_error();

        // Manually add an old error (> 1 minute ago)
        let old_timestamp =
            std::time::Instant::now() - std::time::Duration::from_secs(61);
        monitor.error_count_window.push_back((old_timestamp, 1));

        // Collect metrics should clean old entries
        let metrics = monitor.collect_metrics().await;
        assert_eq!(metrics.error_rate, 1.0); // Only the recent error should count
    }

    #[test]
    fn test_get_process_info() {
        let mut monitor = SystemMonitor::new();
        let info = monitor.get_process_info();

        // Should get info for current process
        assert!(info.is_some());

        if let Some(info) = info {
            assert!(info.cpu_usage >= 0.0);
            assert!(info.memory_bytes > 0);
        }
    }
}
