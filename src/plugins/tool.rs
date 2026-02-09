use std::ffi::OsString;
use std::process::Output;

use super::Plugins;

/// Common interface for a named external CLI tool.
///
/// This is a lightweight “plugin” pattern: each tool provides a small adapter
/// struct that implements this trait, and then exposes typed helper methods.
pub trait NamedTool {
    const NAME: &'static str;

    fn plugins(&self) -> &Plugins;

    fn is_available(&self) -> bool {
        self.plugins().has_in_path(Self::NAME)
    }

    fn output(&self, args: Vec<OsString>) -> std::io::Result<Output> {
        self.plugins().output(Self::NAME, args)
    }

    fn output_if_available(&self, args: Vec<OsString>) -> Option<Output> {
        self.plugins().output_if_available(Self::NAME, args)
    }

    fn probe_success(&self, args: Vec<OsString>) -> bool {
        self.plugins().probe_success(Self::NAME, args)
    }
}
