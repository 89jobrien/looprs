//! Self-updating xtask shim that delegates to the `taskit` binary.
//!
//! Usage: `cargo xtask <subcommand> [args...]`
//!
//! If `taskit` is not installed, this shim installs it automatically
//! via `cargo install taskit`.

use std::process::{Command, exit};
const CLI_BIN_TEST_ARGS: &[&str] = &[
    "nextest",
    "run",
    "--locked",
    "-p",
    "looprs-cli",
    "--bin",
    "looprs",
    "--status-level",
    "none",
    "--final-status-level",
    "fail",
    "--hide-progress-bar",
    "--fail-fast",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let is_pre_push = is_plain_pre_push(&args);

    // Try running taskit directly first
    match Command::new("taskit").args(&args).status() {
        Ok(status) => {
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
            if is_pre_push {
                exit(run_cli_bin_tests());
            }
            exit(0);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("taskit not found, installing via cargo install...");
            let install = Command::new("cargo")
                .args(["install", "taskit"])
                .status()
                .expect("failed to run cargo install");
            if !install.success() {
                eprintln!("failed to install taskit");
                exit(1);
            }
            // Retry after install
            let status = Command::new("taskit")
                .args(&args)
                .status()
                .expect("failed to run taskit after install");
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
            if is_pre_push {
                exit(run_cli_bin_tests());
            }
            exit(0);
        }
        Err(e) => {
            eprintln!("failed to run taskit: {e}");
            exit(1);
        }
    }
}

fn run_cli_bin_tests() -> i32 {
    eprintln!("  --- looprs-cli bin ---");
    let status = Command::new("cargo")
        .args(CLI_BIN_TEST_ARGS)
        .status()
        .expect("failed to run looprs-cli binary tests");

    status.code().unwrap_or(1)
}

fn is_plain_pre_push(args: &[String]) -> bool {
    matches!(args, [subcommand] if subcommand == "pre-push")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn plain_pre_push_runs_cli_bin_tests() {
        assert!(is_plain_pre_push(&args(&["pre-push"])));
    }

    #[test]
    fn non_pre_push_taskit_commands_do_not_run_cli_bin_tests() {
        assert!(!is_plain_pre_push(&args(&["pre-commit"])));
        assert!(!is_plain_pre_push(&args(&["test"])));
        assert!(!is_plain_pre_push(&args(&[])));
    }

    #[test]
    fn pre_push_with_taskit_args_stays_taskit_only() {
        assert!(!is_plain_pre_push(&args(&["pre-push", "--dry-run"])));
    }

    #[test]
    fn cli_bin_test_command_targets_looprs_cli_binary() {
        assert!(CLI_BIN_TEST_ARGS.contains(&"nextest"));
        assert!(CLI_BIN_TEST_ARGS.contains(&"run"));
        assert!(CLI_BIN_TEST_ARGS.contains(&"-p"));
        assert!(CLI_BIN_TEST_ARGS.contains(&"looprs-cli"));
        assert!(CLI_BIN_TEST_ARGS.contains(&"--bin"));
        assert!(CLI_BIN_TEST_ARGS.contains(&"looprs"));
    }
}
