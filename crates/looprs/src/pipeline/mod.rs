pub mod context_compact;
pub mod logging;
pub mod types;

use std::process::Command;

use crate::app_config::PipelineChecksConfig;
use crate::pipeline::types::{PipelineReport, StepResult};

#[derive(Debug, Default)]
pub struct PipelineRunner;

impl PipelineRunner {
    /// Run the configured check suite and return a report.
    ///
    /// Enabled checks are executed in order: build, lint, tests. Each check
    /// runs `cargo <subcommand>` in the current working directory. Checks are
    /// independent — a failure does not skip subsequent checks unless
    /// `fail_fast` behaviour is desired by the caller.
    pub fn run_checks(cfg: &PipelineChecksConfig) -> PipelineReport {
        let mut steps = Vec::new();

        if cfg.run_build {
            steps.push(Self::run_step(
                "build",
                &["build", "--workspace", "--quiet"],
            ));
        }
        if cfg.run_lint {
            steps.push(Self::run_step(
                "lint",
                &["clippy", "--workspace", "--quiet", "--", "-D", "warnings"],
            ));
        }
        if cfg.run_tests {
            // Prefer cargo-nextest if available; fall back to cargo test.
            let ok = Command::new("cargo")
                .args(["nextest", "run", "--workspace", "--quiet"])
                .status()
                .or_else(|_| {
                    Command::new("cargo")
                        .args(["test", "--workspace", "--quiet"])
                        .status()
                })
                .map(|s| s.success())
                .unwrap_or(false);
            steps.push(StepResult {
                step: "tests".into(),
                success: ok,
            });
        }
        if cfg.run_typecheck {
            steps.push(Self::run_step(
                "typecheck",
                &["check", "--workspace", "--quiet"],
            ));
        }

        PipelineReport {
            steps,
            tools: vec![],
            reward: None,
        }
    }

    fn run_step(name: &str, cargo_args: &[&str]) -> StepResult {
        let ok = Command::new("cargo")
            .args(cargo_args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        StepResult {
            step: name.to_string(),
            success: ok,
        }
    }

    #[cfg(test)]
    fn run_step_cmd(name: &str, program: &str, args: &[&str]) -> StepResult {
        let ok = Command::new(program)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        StepResult {
            step: name.to_string(),
            success: ok,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_disabled() -> PipelineChecksConfig {
        PipelineChecksConfig {
            run_build: false,
            run_tests: false,
            run_lint: false,
            run_typecheck: false,
            run_bench: false,
        }
    }

    #[test]
    fn no_steps_when_all_disabled() {
        let report = PipelineRunner::run_checks(&all_disabled());
        assert!(
            report.steps.is_empty(),
            "expected empty steps, got {:?}",
            report.steps
        );
    }

    #[test]
    fn typecheck_step_name_and_success() {
        let cfg = PipelineChecksConfig {
            run_typecheck: true,
            ..all_disabled()
        };
        let report = PipelineRunner::run_checks(&cfg);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "typecheck");
        assert!(
            report.steps[0].success,
            "cargo check should pass on clean workspace"
        );
    }

    #[test]
    fn step_order_matches_config() {
        let cfg = PipelineChecksConfig {
            run_build: true,
            run_typecheck: true,
            ..all_disabled()
        };
        let report = PipelineRunner::run_checks(&cfg);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.steps[0].step, "build");
        assert_eq!(report.steps[1].step, "typecheck");
    }

    #[test]
    fn step_failure_recorded() {
        let result = PipelineRunner::run_step_cmd("probe", "false", &[]);
        assert_eq!(result.step, "probe");
        assert!(!result.success, "`false` must produce a failed step");
    }

    #[test]
    fn report_tools_and_reward_default() {
        let report = PipelineRunner::run_checks(&all_disabled());
        assert!(report.tools.is_empty());
        assert!(report.reward.is_none());
    }
}
