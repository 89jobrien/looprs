//! Verifies the binary's command-line help, version, and argument error contract.

use std::process::Command;

fn looprs(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_looprs"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("PROVIDER", "provider-that-must-not-start")
        .output()
        .expect("run looprs")
}

#[test]
fn help_flags_exit_successfully_without_starting_a_provider() {
    for flag in ["-h", "--help"] {
        let output = looprs(&[flag]);
        assert!(output.status.success(), "{flag}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stdout.contains("Usage: looprs"), "stdout: {stdout:?}");
        assert!(stdout.contains("--machine-protocol"), "stdout: {stdout:?}");
        assert!(stderr.is_empty(), "stderr: {stderr:?}");
    }
}

#[test]
fn version_flags_exit_successfully_without_starting_a_provider() {
    for flag in ["-V", "--version"] {
        let output = looprs(&[flag]);
        assert!(output.status.success(), "{flag}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stdout.trim(),
            format!("looprs {}", env!("CARGO_PKG_VERSION"))
        );
        assert!(stderr.is_empty(), "stderr: {stderr:?}");
    }
}

#[test]
fn help_wins_when_both_meta_flags_are_present() {
    let output = looprs(&["--version", "--help"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage: looprs"));
}
