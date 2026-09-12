pub mod context_compact;
pub mod logging;
pub mod types;

use std::process::Command;

use crate::app_config::{PipelineChecksConfig, PipelineConfig};
use crate::pipeline::logging::PipelineLogger;
use crate::pipeline::types::{PipelineReport, RewardReport, StepResult, ToolResult};

#[derive(Debug, Default)]
pub struct PipelineRunner;

impl PipelineRunner {
    /// Run the complete configured pipeline.
    pub fn run(cfg: &PipelineConfig) -> PipelineReport {
        let cargo_available = Command::new("cargo").arg("--version").output().is_ok();
        let mut report = if cfg.require_tools && !cargo_available {
            PipelineReport {
                steps: vec![StepResult {
                    step: "required-tool:cargo".to_string(),
                    success: false,
                }],
                tools: Vec::new(),
                reward: None,
            }
        } else {
            Self::run_checks_with(&cfg.checks, cfg.fail_fast, Self::run_step)
        };

        report.tools.push(ToolResult {
            tool: "cargo".to_string(),
            output: serde_json::json!({"available": cargo_available}),
        });
        let reward = Self::reward(&report.steps);
        report.reward = Some(RewardReport { reward });

        if let Ok(logger) = PipelineLogger::new(cfg.log_dir.clone().into()) {
            for step in &report.steps {
                let _ = logger.log_event("check", serde_json::json!(step));
            }
            let _ = logger.log_event(
                "complete",
                serde_json::json!({
                    "reward": reward,
                    "threshold": cfg.reward_threshold,
                    "success": report.succeeds(cfg.reward_threshold),
                }),
            );
        }

        report
    }

    /// Run the configured check suite and return a report.
    ///
    /// Enabled checks are executed in order: build, lint, tests. Each check
    /// runs `cargo <subcommand>` in the current working directory. Checks are
    /// independent — a failure does not skip subsequent checks unless
    /// `fail_fast` behaviour is desired by the caller.
    pub fn run_checks(cfg: &PipelineChecksConfig) -> PipelineReport {
        Self::run_checks_with(cfg, false, Self::run_step)
    }

    fn run_checks_with<F>(
        cfg: &PipelineChecksConfig,
        fail_fast: bool,
        mut run_step: F,
    ) -> PipelineReport
    where
        F: FnMut(&str, &[&str]) -> StepResult,
    {
        let mut steps = Vec::new();

        if cfg.run_build {
            steps.push(run_step("build", &["build", "--workspace", "--quiet"]));
            if fail_fast && !steps.last().is_some_and(|step| step.success) {
                return Self::report(steps);
            }
        }
        if cfg.run_lint {
            steps.push(run_step(
                "lint",
                &["clippy", "--workspace", "--quiet", "--", "-D", "warnings"],
            ));
            if fail_fast && !steps.last().is_some_and(|step| step.success) {
                return Self::report(steps);
            }
        }
        if cfg.run_tests {
            steps.push(run_step(
                "tests",
                &["nextest", "run", "--workspace", "--quiet"],
            ));
            if fail_fast && !steps.last().is_some_and(|step| step.success) {
                return Self::report(steps);
            }
        }
        if cfg.run_typecheck {
            steps.push(run_step("typecheck", &["check", "--workspace", "--quiet"]));
            if fail_fast && !steps.last().is_some_and(|step| step.success) {
                return Self::report(steps);
            }
        }
        if cfg.run_bench {
            steps.push(run_step(
                "bench",
                &["bench", "--workspace", "--no-run", "--quiet"],
            ));
        }

        Self::report(steps)
    }

    fn report(steps: Vec<StepResult>) -> PipelineReport {
        PipelineReport {
            steps,
            tools: vec![],
            reward: None,
        }
    }

    fn reward(steps: &[StepResult]) -> f64 {
        if steps.is_empty() {
            return 1.0;
        }
        steps.iter().filter(|step| step.success).count() as f64 / steps.len() as f64
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

    #[test]
    fn benchmark_setting_adds_benchmark_step() {
        let cfg = PipelineChecksConfig {
            run_bench: true,
            ..all_disabled()
        };

        let report = PipelineRunner::run_checks_with(&cfg, false, |name, _| StepResult {
            step: name.to_string(),
            success: true,
        });

        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "bench");
    }

    #[test]
    fn fail_fast_stops_after_first_failure() {
        let cfg = PipelineChecksConfig {
            run_build: true,
            run_lint: true,
            run_tests: true,
            ..all_disabled()
        };

        let report = PipelineRunner::run_checks_with(&cfg, true, |name, _| StepResult {
            step: name.to_string(),
            success: name != "lint",
        });

        assert_eq!(
            report
                .steps
                .iter()
                .map(|step| step.step.as_str())
                .collect::<Vec<_>>(),
            ["build", "lint"]
        );
    }

    #[test]
    fn reward_is_successful_step_ratio() {
        let steps = [
            StepResult {
                step: "build".to_string(),
                success: true,
            },
            StepResult {
                step: "lint".to_string(),
                success: false,
            },
        ];

        assert_eq!(PipelineRunner::reward(&steps), 0.5);
    }
}
