//! PluginExecutor port — abstraction over named CLI tool execution.

use std::ffi::OsString;
use std::process::Output;

/// Port: Execute named CLI tools (plugins).
///
/// Abstracts plugin execution so the domain layer can request tool
/// execution without knowing about subprocess details or PATH resolution.
pub trait PluginExecutor: Send + Sync {
    fn has_tool(&self, tool: &str) -> bool;
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
