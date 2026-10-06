//! Self-updating xtask shim that delegates to the `taskit` binary.
//!
//! Usage: `cargo xtask <subcommand> [args...]`
//!
//! If `taskit` is not installed, this shim installs it automatically
//! via `cargo install taskit`.

use std::process::{Command, exit};

#[derive(Debug, PartialEq, Eq)]
struct CargoGate {
    name: &'static str,
    args: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
}

const DOCTEST_ARGS: &[&str] = &["test", "--locked", "--workspace", "--doc", "--all-features"];
const RUSTDOC_ARGS: &[&str] = &[
    "doc",
    "--locked",
    "--workspace",
    "--no-deps",
    "--all-features",
];
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
const PRE_PUSH_GATES: &[CargoGate] = &[
    CargoGate {
        name: "workspace doctests",
        args: DOCTEST_ARGS,
        env: &[],
    },
    CargoGate {
        name: "rustdoc warnings",
        args: RUSTDOC_ARGS,
        env: &[("RUSTDOCFLAGS", "-D warnings")],
    },
    CargoGate {
        name: "looprs-cli bin",
        args: CLI_BIN_TEST_ARGS,
        env: &[],
    },
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if is_plain_install(&args) {
        exit(run_install());
    }

    let is_pre_push = is_plain_pre_push(&args);

    // Try running taskit directly first
    match Command::new("taskit").args(&args).status() {
        Ok(status) => {
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
            if is_pre_push {
                exit(run_pre_push_gates());
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
                exit(run_pre_push_gates());
            }
            exit(0);
        }
        Err(e) => {
            eprintln!("failed to run taskit: {e}");
            exit(1);
        }
    }
}

fn run_pre_push_gates() -> i32 {
    run_gate_commands(PRE_PUSH_GATES, run_cargo_gate)
}

fn run_gate_commands(gates: &[CargoGate], mut run: impl FnMut(&CargoGate) -> i32) -> i32 {
    for gate in gates {
        let code = run(gate);
        if code != 0 {
            return code;
        }
    }
    0
}

fn run_cargo_gate(gate: &CargoGate) -> i32 {
    eprintln!("  --- {} ---", gate.name);
    let mut command = Command::new("cargo");
    command.args(gate.args);
    for (key, value) in gate.env {
        command.env(key, value);
    }

    match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("failed to run {}: {error}", gate.name);
            1
        }
    }
}

fn is_plain_pre_push(args: &[String]) -> bool {
    matches!(args, [subcommand] if subcommand == "pre-push")
        || matches!(args, [group, subcommand] if group == "check" && subcommand == "pre-push")
}

/// `taskit self install` installs taskit itself, not looprs — intercept
/// `install` here instead of delegating, since only this workspace knows
/// which crate produces the `looprs` binary.
fn is_plain_install(args: &[String]) -> bool {
    matches!(args, [subcommand] if subcommand == "install")
}

fn run_install() -> i32 {
    let status = Command::new("cargo")
        .args(["install", "--path", "crates/looprs-cli", "--force"])
        .status()
        .expect("failed to run cargo install");
    status.code().unwrap_or(1)
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
        assert!(is_plain_pre_push(&args(&["check", "pre-push"])));
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
        assert!(!is_plain_pre_push(&args(&[
            "check",
            "pre-push",
            "--dry-run"
        ])));
    }

    #[test]
    fn plain_install_is_intercepted() {
        assert!(is_plain_install(&args(&["install"])));
    }

    #[test]
    fn non_plain_install_falls_through_to_taskit() {
        assert!(!is_plain_install(&args(&["dev", "install"])));
        assert!(!is_plain_install(&args(&["install", "--dry-run"])));
        assert!(!is_plain_install(&args(&[])));
    }

    #[test]
    fn pre_push_gate_commands_cover_doctests_rustdoc_and_cli_binary() {
        assert_eq!(PRE_PUSH_GATES.len(), 3);
        assert_eq!(PRE_PUSH_GATES[0].args, DOCTEST_ARGS);
        assert_eq!(PRE_PUSH_GATES[1].args, RUSTDOC_ARGS);
        assert_eq!(PRE_PUSH_GATES[1].env, &[("RUSTDOCFLAGS", "-D warnings")]);
        assert_eq!(PRE_PUSH_GATES[2].args, CLI_BIN_TEST_ARGS);
    }

    #[test]
    fn pre_push_gates_stop_and_propagate_the_first_failure() {
        let mut executed = Vec::new();

        let code = run_gate_commands(PRE_PUSH_GATES, |gate| {
            executed.push(gate.name);
            if gate.name == "rustdoc warnings" {
                23
            } else {
                0
            }
        });

        assert_eq!(code, 23);
        assert_eq!(executed, ["workspace doctests", "rustdoc warnings"]);
    }
}
