//! PluginExecutor port adapter — bridges domain to Plugins infrastructure.
//!
//! This adapter implements the PluginExecutor port using the existing Plugins
//! system (Runner + ToolRegistry). It provides a domain-facing interface
//! without exposing low-level subprocess details.

use std::ffi::OsString;
use std::process::Output;

use crate::plugins::Plugins;
use crate::ports::PluginExecutor;

/// Adapter implementing the PluginExecutor port via the Plugins system.
///
/// Wraps a reference to Plugins to provide a domain-facing interface.
pub struct PluginsAdapter<'a> {
    plugins: &'a Plugins,
}

impl<'a> PluginsAdapter<'a> {
    /// Create a new adapter wrapping a Plugins reference.
    pub fn new(plugins: &'a Plugins) -> Self {
        Self { plugins }
    }

    /// Create an adapter using the system-wide Plugins singleton.
    pub fn system() -> Self {
        Self {
            plugins: Plugins::system(),
        }
    }
}

impl<'a> PluginExecutor for PluginsAdapter<'a> {
    fn has_tool(&self, tool: &str) -> bool {
        self.plugins.has_in_path(tool)
    }

    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output> {
        self.plugins.output(tool, args)
    }

    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output> {
        self.plugins.output_if_available(tool, args)
    }

    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool {
        self.plugins.probe_success(tool, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_has_tool_to_has_in_path() {
        let adapter = PluginsAdapter::system();
        // /bin/echo should exist on most unix systems
        assert!(adapter.has_tool("echo"));
        assert!(!adapter.has_tool("nonexistent_tool_xyz_12345"));
    }

    #[test]
    fn adapts_probe_tool_success() {
        let adapter = PluginsAdapter::system();
        // true is a builtin in most shells, but we test with a standard tool
        assert!(adapter.probe_tool_success("echo", vec![]));
    }
}
