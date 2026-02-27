//! Process manager trait for dependency injection and testing.
//!
//! This module defines the `ProcessManager` trait that abstracts process lifecycle
//! management, allowing for easy mocking and testing without real process spawning.

use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Health status of a managed process.
#[derive(Debug, Clone)]
pub struct ProcessHealth {
    pub alive: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
}

/// Trait for managing process lifecycle.
///
/// This trait abstracts the underlying process implementation, allowing for
/// easy mocking in tests and alternative implementations in production.
#[async_trait]
pub trait ProcessManager: Send + Debug {
    /// Start the process with the given arguments.
    ///
    /// Returns the PID and start timestamp on success.
    async fn start(&mut self, args: Vec<String>) -> Result<(u32, i64)>;

    /// Stop the process, either gracefully or forcefully.
    ///
    /// Returns the exit code and exit timestamp.
    async fn stop(&mut self, force: bool) -> Result<(Option<i32>, i64)>;

    /// Check if the process is currently running.
    fn is_running(&self) -> bool;

    /// Get the process ID if running.
    fn pid(&self) -> Option<u32>;

    /// Get the process uptime in seconds.
    fn uptime_secs(&self) -> Option<u64>;

    /// Get the full health status of the process.
    fn health(&self) -> ProcessHealth {
        ProcessHealth {
            alive: self.is_running(),
            pid: self.pid(),
            uptime_secs: self.uptime_secs(),
        }
    }
}
