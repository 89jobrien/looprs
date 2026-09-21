//! PluginExecutor port — abstraction over named CLI tool execution.

use std::ffi::OsString;
use std::process::Output;

/// Port: Execute named CLI tools (plugins).
///
/// Abstracts plugin execution so the domain layer can request tool
/// execution without knowing about subprocess details or PATH resolution.
pub trait PluginExecutor: Send + Sync {
    /// Returns whether the named executable is available on `PATH`.
    fn has_tool(&self, tool: &str) -> bool;
    /// Executes the named tool with the supplied arguments.
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;
    /// Executes the tool, returning `None` if resolution or execution fails.
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;
    /// Returns whether the tool executes with a successful exit status.
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
