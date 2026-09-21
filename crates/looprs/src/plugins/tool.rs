//! Shared operations for typed adapters around named external CLI tools.

use std::ffi::OsString;
use std::process::Output;

use super::Plugins;

/// Common interface for a named external CLI tool.
///
/// This is a lightweight “plugin” pattern: each tool provides a small adapter
/// struct that implements this trait, and then exposes typed helper methods.
pub trait NamedTool {
    const NAME: &'static str;

    /// Returns the plugin registry and runner used by this adapter.
    fn plugins(&self) -> &Plugins;

    /// Returns whether the named executable resolves through the plugin registry.
    fn is_available(&self) -> bool {
        self.plugins().has_in_path(Self::NAME)
    }

    /// Runs the named executable with `args` and captures its output.
    fn output(&self, args: Vec<OsString>) -> std::io::Result<Output> {
        self.plugins().output(Self::NAME, args)
    }

    /// Attempts to run the executable, returning `None` if resolution or execution fails.
    fn output_if_available(&self, args: Vec<OsString>) -> Option<Output> {
        self.plugins().output_if_available(Self::NAME, args)
    }

    /// Returns whether the executable runs and exits successfully.
    fn probe_success(&self, args: Vec<OsString>) -> bool {
        self.plugins().probe_success(Self::NAME, args)
    }
}
