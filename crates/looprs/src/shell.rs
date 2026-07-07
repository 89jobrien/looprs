use std::process::{Command, Output, Stdio};

pub const NUSHELL_BIN: &str = "nu";
pub const BASH_BIN: &str = "bash";

pub fn run_nu_command(command: &str) -> std::io::Result<Output> {
    Command::new(NUSHELL_BIN)
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

pub fn run_bash_command(command: &str) -> std::io::Result<Output> {
    Command::new(BASH_BIN)
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
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
}
