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
    /// Run the pipeline using all configured execution semantics.
    pub fn run(cfg: &PipelineConfig) -> PipelineReport {
        Self::run_with(
            cfg,
            |program, args| {
                Command::new(program)
                    .args(args)
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
            },
            |tool| {
                Command::new(tool)
                    .arg("--version")
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
            },
        )
    }

    /// Run only check toggles with default execution policy.
    pub fn run_checks(cfg: &PipelineChecksConfig) -> PipelineReport {
        let cfg = PipelineConfig {
            checks: cfg.clone(),
            ..PipelineConfig::default()
        };
        Self::run(&cfg)
    }

    /// A failed pipeline blocks the agent only when `block_on_failure` is set.
    pub fn should_block(cfg: &PipelineConfig, report: &PipelineReport) -> bool {
        cfg.block_on_failure && report.steps.iter().any(|step| !step.success)
    }

    fn run_with<C, A>(
        cfg: &PipelineConfig,
        mut run_command: C,
        mut tool_available: A,
    ) -> PipelineReport
    where
        C: FnMut(&str, &[&str]) -> bool,
        A: FnMut(&str) -> bool,
    {
        let mut steps = Vec::new();
        let mut tools = Vec::new();
        let logger = PipelineLogger::new(cfg.log_dir.clone().into()).ok();
        let checks_enabled = cfg.checks.run_build
            || cfg.checks.run_tests
            || cfg.checks.run_lint
            || cfg.checks.run_typecheck
            || cfg.checks.run_bench;

        let cargo_available = !checks_enabled || tool_available("cargo");
        if checks_enabled {
            tools.push(Self::tool_result("cargo", cargo_available, true));
        }

        let nextest_available = cfg.checks.run_tests && tool_available("cargo-nextest");
        if cfg.checks.run_tests {
            tools.push(Self::tool_result("cargo-nextest", nextest_available, false));
        }

        if cfg.require_tools && !cargo_available {
            Self::record_step(
                &mut steps,
                &logger,
                StepResult {
                    step: "required_tool:cargo".into(),
                    success: false,
                },
            );
        } else {
            let test_args = if nextest_available {
                vec!["nextest", "run", "--workspace", "--quiet"]
            } else {
                vec!["test", "--workspace", "--quiet"]
            };
            let configured_steps = [
                (
                    cfg.checks.run_build,
                    "build",
                    vec!["build", "--workspace", "--quiet"],
                ),
                (cfg.checks.run_tests, "tests", test_args),
                (
                    cfg.checks.run_lint,
                    "lint",
                    vec!["clippy", "--workspace", "--quiet", "--", "-D", "warnings"],
                ),
                (
                    cfg.checks.run_typecheck,
                    "typecheck",
                    vec!["check", "--workspace", "--quiet"],
                ),
                (
                    cfg.checks.run_bench,
                    "bench",
                    vec!["bench", "--workspace", "--no-run", "--quiet"],
                ),
            ];

            for (enabled, name, args) in configured_steps {
                if !enabled {
                    continue;
                }
                let result = StepResult {
                    step: name.into(),
                    success: run_command("cargo", &args),
                };
                let failed = !result.success;
                Self::record_step(&mut steps, &logger, result);
                if failed && cfg.fail_fast {
                    break;
                }
            }
        }

        let attempted = steps
            .iter()
            .filter(|step| !step.step.starts_with("required_tool:"))
            .count();
        let successful = steps
            .iter()
            .filter(|step| !step.step.starts_with("required_tool:") && step.success)
            .count();
        let reward = if attempted == 0 {
            if steps.is_empty() { 1.0 } else { 0.0 }
        } else {
            successful as f64 / attempted as f64
        };
        let threshold = f64::from(cfg.reward_threshold.clamp(0.0, 1.0));
        if reward < threshold {
            Self::record_step(
                &mut steps,
                &logger,
                StepResult {
                    step: "reward_threshold".into(),
                    success: false,
                },
            );
        }

        let report = PipelineReport {
            steps,
            tools,
            reward: Some(RewardReport { reward }),
        };
        if let Some(logger) = logger {
            let data = serde_json::to_value(&report)
                .unwrap_or_else(|error| serde_json::json!({"error": error.to_string()}));
            let _ = logger.log_event("pipeline_complete", data);
        }
        report
    }

    fn tool_result(tool: &str, available: bool, required: bool) -> ToolResult {
        ToolResult {
            tool: tool.into(),
            output: serde_json::json!({
                "available": available,
                "required": required,
            }),
        }
    }

    fn record_step(
        steps: &mut Vec<StepResult>,
        logger: &Option<PipelineLogger>,
        result: StepResult,
    ) {
        if let Some(logger) = logger {
            let _ = logger.log_event(&result.step, serde_json::json!({"success": result.success}));
        }
        steps.push(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::PipelineConfig;
    use std::cell::RefCell;

    fn all_disabled_checks() -> PipelineChecksConfig {
        PipelineChecksConfig {
            run_build: false,
            run_tests: false,
            run_lint: false,
            run_typecheck: false,
            run_bench: false,
        }
    }

    fn config(log_dir: &std::path::Path) -> PipelineConfig {
        PipelineConfig {
            log_dir: log_dir.to_string_lossy().into_owned(),
            checks: all_disabled_checks(),
            ..PipelineConfig::default()
        }
    }

    #[test]
    fn no_steps_when_all_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let report = PipelineRunner::run_with(&config(dir.path()), |_, _| true, |_| true);
        assert!(report.steps.is_empty());
        assert_eq!(report.reward.unwrap().reward, 1.0);
    }

    #[test]
    fn benchmark_setting_executes_benchmark_gate() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.checks.run_bench = true;
        let commands = RefCell::new(Vec::new());

        let report = PipelineRunner::run_with(
            &cfg,
            |program, args| {
                commands
                    .borrow_mut()
                    .push((program.to_string(), args.join(" ")));
                true
            },
            |_| true,
        );

        assert_eq!(report.steps[0].step, "bench");
        assert_eq!(
            commands.into_inner(),
            [("cargo".into(), "bench --workspace --no-run --quiet".into())]
        );
    }

    #[test]
    fn fail_fast_stops_after_first_failed_check() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.fail_fast = true;
        cfg.checks.run_build = true;
        cfg.checks.run_lint = true;
        let calls = RefCell::new(0);

        let report = PipelineRunner::run_with(
            &cfg,
            |_, _| {
                *calls.borrow_mut() += 1;
                false
            },
            |_| true,
        );

        assert_eq!(*calls.borrow(), 1);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "build");
    }

    #[test]
    fn required_tool_preflight_fails_without_running_checks() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.require_tools = true;
        cfg.checks.run_build = true;

        let report = PipelineRunner::run_with(
            &cfg,
            |_, _| panic!("checks must not execute without required tools"),
            |_| false,
        );

        assert_eq!(report.steps[0].step, "required_tool:cargo");
        assert!(!report.steps[0].success);
        assert_eq!(report.tools[0].tool, "cargo");
        assert_eq!(report.tools[0].output["available"], false);
    }

    #[test]
    fn reward_threshold_marks_pipeline_failed() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.reward_threshold = 0.75;
        cfg.checks.run_build = true;
        cfg.checks.run_lint = true;
        let outcomes = RefCell::new(vec![true, false].into_iter());

        let report =
            PipelineRunner::run_with(&cfg, |_, _| outcomes.borrow_mut().next().unwrap(), |_| true);

        assert_eq!(report.reward.as_ref().unwrap().reward, 0.5);
        assert_eq!(report.steps.last().unwrap().step, "reward_threshold");
        assert!(!report.steps.last().unwrap().success);
    }

    #[test]
    fn configured_log_directory_receives_step_events() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.checks.run_build = true;

        PipelineRunner::run_with(&cfg, |_, _| true, |_| true);

        let entries = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(entries.contains("\"step\":\"build\""));
        assert!(entries.contains("\"step\":\"pipeline_complete\""));
    }

    #[test]
    fn failure_policy_only_blocks_when_configured() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.checks.run_build = true;
        let report = PipelineRunner::run_with(&cfg, |_, _| false, |_| true);

        assert!(!PipelineRunner::should_block(&cfg, &report));
        cfg.block_on_failure = true;
        assert!(PipelineRunner::should_block(&cfg, &report));
    }
}
