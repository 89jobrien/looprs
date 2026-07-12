use std::io;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

pub const NUSHELL_BIN: &str = "nu";
pub const BASH_BIN: &str = "bash";

pub fn run_nu_command(command: &str) -> io::Result<Output> {
    run_nu_command_with_timeout(command, None)
}

pub fn run_bash_command(command: &str) -> io::Result<Output> {
    run_bash_command_with_timeout(command, None)
}

/// Run a Nushell command with an optional wall-clock timeout.
///
/// Returns `Err` with `ErrorKind::TimedOut` if the process exceeds `timeout`.
pub fn run_nu_command_with_timeout(command: &str, timeout: Option<Duration>) -> io::Result<Output> {
    run_with_timeout(NUSHELL_BIN, &["-c", command], timeout)
}

/// Run a Bash command with an optional wall-clock timeout.
pub fn run_bash_command_with_timeout(
    command: &str,
    timeout: Option<Duration>,
) -> io::Result<Output> {
    run_with_timeout(BASH_BIN, &["-c", command], timeout)
}

fn run_with_timeout(bin: &str, args: &[&str], timeout: Option<Duration>) -> io::Result<Output> {
    let mut child = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    match timeout {
        None => child.wait_with_output(),
        Some(dur) => {
            // Poll until the process exits or the deadline passes.
            let deadline = std::time::Instant::now() + dur;
            loop {
                match child.try_wait()? {
                    Some(_) => return child.wait_with_output(),
                    None => {
                        if std::time::Instant::now() >= deadline {
                            let _ = child.kill();
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                format!("command timed out after {}s", dur.as_secs()),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_nushell_command() {
        let output = run_nu_command("\"ok\"").expect("nu should run");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("ok"));
    }

    #[test]
    fn runs_bash_command() {
        let output = run_bash_command("echo ok").expect("bash should run");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("ok"));
    }

    #[test]
    fn timeout_kills_long_running_command() {
        let err = run_bash_command_with_timeout("sleep 10", Some(Duration::from_millis(200)))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn no_timeout_completes_normally() {
        let out = run_bash_command_with_timeout("echo done", None).unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("done"));
    }
}
