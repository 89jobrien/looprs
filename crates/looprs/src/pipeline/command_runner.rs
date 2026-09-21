//! Runs external pipeline commands and distinguishes spawn failures from non-zero exits.

use std::io;
use std::process::Command;

/// Executes one pipeline command and reports whether it exited successfully.
///
/// Returning an [`io::Error`] distinguishes a process that could not be spawned
/// from one that ran and returned a non-zero status.
///
/// # Example
///
/// ```
/// use looprs::pipeline::PipelineCommandRunner;
///
/// struct AlwaysPass;
/// impl PipelineCommandRunner for AlwaysPass {
///     fn run(&mut self, _program: &str, _args: &[&str]) -> std::io::Result<bool> {
///         Ok(true)
///     }
/// }
///
/// assert!(AlwaysPass.run("cargo", &["check"])?);
/// # Ok::<(), std::io::Error>(())
/// ```
pub trait PipelineCommandRunner {
    /// Run `program` with `args`, preserving spawn errors for fallback logic.
    fn run(&mut self, program: &str, args: &[&str]) -> io::Result<bool>;
}

/// Command runner backed by [`std::process::Command`].
#[derive(Debug, Default)]
pub struct ProcessCommandRunner;

impl PipelineCommandRunner for ProcessCommandRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> io::Result<bool> {
        Command::new(program)
            .args(args)
            .status()
            .map(|status| status.success())
    }
}
