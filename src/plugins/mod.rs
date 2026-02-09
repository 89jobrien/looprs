mod registry;
mod resolve;
mod runner;

use std::ffi::OsString;
use std::process::Output;
use std::sync::{Arc, OnceLock};

pub use registry::{ToolRegistry, ToolResolver};
pub use resolve::PathResolver;
pub use runner::{MockRunner, OsRunner, Runner};

/// Central access point for external CLI tools.
///
/// Scope: named tool adapters + availability checks.
/// Non-goal: general-purpose `sh -c` execution (hooks/commands keep their own).
pub struct Plugins {
    runner: Arc<dyn Runner>,
    registry: ToolRegistry,
}

impl Plugins {
    pub fn new(runner: Arc<dyn Runner>, resolver: Arc<dyn ToolResolver>) -> Self {
        Self {
            runner,
            registry: ToolRegistry::new(resolver),
        }
    }

    pub fn system() -> &'static Plugins {
        static INSTANCE: OnceLock<Plugins> = OnceLock::new();
        INSTANCE.get_or_init(|| Plugins::new(Arc::new(OsRunner), Arc::new(PathResolver)))
    }

    /// PATH-based presence check (no subprocess execution).
    pub fn has_in_path(&self, tool: &str) -> bool {
        self.registry.has(tool)
    }

    /// Resolve a tool via PATH and execute it.
    pub fn output(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output> {
        let program = self.registry.require(tool)?;
        self.runner.output(&program, &args)
    }

    /// Execute a tool if it is present in PATH; otherwise return None.
    pub fn output_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output> {
        self.output(tool, args).ok()
    }

    /// Execute and require exit status success.
    pub fn probe_success(&self, tool: &str, args: Vec<OsString>) -> bool {
        let Some(out) = self.output_if_available(tool, args) else {
            return false;
        };
        out.status.success()
    }
}

pub fn system() -> &'static Plugins {
    Plugins::system()
}
