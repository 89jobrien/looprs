use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

pub trait Runner: Send + Sync {
    fn output(&self, program: &Path, args: &[OsString]) -> std::io::Result<Output>;
}

pub struct OsRunner;

impl Runner for OsRunner {
    fn output(&self, program: &Path, args: &[OsString]) -> std::io::Result<Output> {
        Command::new(program).args(args).output()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCall {
    pub program: std::path::PathBuf,
    pub args: Vec<OsString>,
}

/// Simple test double that records calls and can return canned outputs.
///
/// Intended for unit tests of plugins/consumers.
pub struct MockRunner {
    calls: Arc<Mutex<Vec<RunCall>>>,
    outputs: Arc<Mutex<Vec<std::io::Result<Output>>>>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            outputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push_output(&self, out: std::io::Result<Output>) {
        self.outputs.lock().unwrap().push(out);
    }

    pub fn calls(&self) -> Vec<RunCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl Default for MockRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for MockRunner {
    fn output(&self, program: &Path, args: &[OsString]) -> std::io::Result<Output> {
        self.calls.lock().unwrap().push(RunCall {
            program: program.to_path_buf(),
            args: args.to_vec(),
        });

        let mut outputs = self.outputs.lock().unwrap();
        if outputs.is_empty() {
            return Err(std::io::Error::other("MockRunner has no queued outputs"));
        }
        outputs.remove(0)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn output_ok(stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn mock_runner_records_calls_and_returns_outputs() {
        let mr = MockRunner::new();
        mr.push_output(Ok(output_ok("ok")));

        let out = mr
            .output(
                Path::new("/bin/echo"),
                &[OsString::from("hello"), OsString::from("world")],
            )
            .unwrap();

        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "ok");

        let calls = mr.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].program, std::path::PathBuf::from("/bin/echo"));
        assert_eq!(calls[0].args.len(), 2);
    }
}
