//! PluginExecutor port — abstraction over named CLI tool execution.

use std::ffi::OsString;
use std::process::Output;

/// Port: Execute named CLI tools (plugins).
///
/// Abstracts plugin execution so the domain layer can request tool
/// execution without knowing about subprocess details or PATH resolution.
pub trait PluginExecutor: Send + Sync {
    /// Return `true` when `tool` is available for execution.
    fn has_tool(&self, tool: &str) -> bool;
    /// Execute `tool` with `args`, returning subprocess output.
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;
    /// Execute `tool` only when present; otherwise return `None`.
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;
    /// Run a probe command and return whether it succeeded.
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
