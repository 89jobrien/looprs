//! Defines shared execution helpers for typed external-tool adapters.

use std::ffi::OsString;
use std::process::Output;

use super::Plugins;

/// Common interface for a named external CLI tool.
///
/// This is a lightweight “plugin” pattern: each tool provides a small adapter
/// struct that implements this trait, and then exposes typed helper methods.
pub trait NamedTool {
    const NAME: &'static str;

    /// Returns the plugin service used to resolve and run this tool.
    fn plugins(&self) -> &Plugins;

    /// Returns whether this tool's executable is present on PATH.
    fn is_available(&self) -> bool {
        self.plugins().has_in_path(Self::NAME)
    }

    /// Executes this tool with the supplied arguments and captures its output.
    fn output(&self, args: Vec<OsString>) -> std::io::Result<Output> {
        self.plugins().output(Self::NAME, args)
    }

    /// Executes this tool when available, returning `None` otherwise.
    fn output_if_available(&self, args: Vec<OsString>) -> Option<Output> {
        self.plugins().output_if_available(Self::NAME, args)
    }

    /// Returns whether invoking this tool with the supplied arguments succeeds.
    fn probe_success(&self, args: Vec<OsString>) -> bool {
        self.plugins().probe_success(Self::NAME, args)
    }
}
