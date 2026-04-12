//! Ports (hexagonal architecture) — outbound interfaces to external systems.
//!
//! Ports define what the application domain needs from external infrastructure,
//! not how those needs are fulfilled. Adapters (in impl/) provide concrete implementations.

use std::ffi::OsString;
use std::process::Output;

/// Port: Execute named CLI tools (plugins).
///
/// This port abstracts plugin execution, allowing the domain layer to request
/// tool execution without knowing about subprocess details, path resolution, or
/// tool availability probing.
///
/// Implementors must:
/// - Handle tool resolution (PATH lookup)
/// - Execute the tool with arguments
/// - Return process output or errors
pub trait PluginExecutor: Send + Sync {
    /// Check if a named tool is available in PATH.
    ///
    /// This is a fast, non-execution check for tool presence.
    fn has_tool(&self, tool: &str) -> bool;

    /// Execute a named tool, requiring it to exist.
    ///
    /// Returns an error if the tool is not found in PATH.
    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output>;

    /// Execute a named tool if it exists; otherwise return None.
    ///
    /// This is the "soft" version of `execute_tool` for optional tools.
    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output>;

    /// Probe if a named tool exists and succeeds with given arguments.
    ///
    /// Returns true if the tool is available and exits with status 0.
    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool;
}
