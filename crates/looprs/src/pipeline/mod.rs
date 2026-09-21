mod command_runner;
pub mod context_compact;
pub mod logging;
pub mod types;
mod worktree_transaction;

pub(crate) use worktree_transaction::WorktreeTransaction;

use std::io;

use crate::app_config::{PipelineChecksConfig, PipelineConfig};
use crate::pipeline::logging::PipelineLogger;
use crate::pipeline::types::{PipelineReport, RewardReport, StepResult, ToolResult};
pub use command_runner::{PipelineCommandRunner, ProcessCommandRunner};
pub use logging::PipelineEventSink;

/// Executes legacy check suites and expanded configured pipelines.
#[derive(Debug, Default)]
pub struct PipelineRunner;

impl PipelineRunner {
    /// Run the expanded pipeline with tool discovery, rewards, and JSONL events.
    ///
    /// Command spawn failures and event-log failures are returned as failed
    /// report steps rather than panics.
    pub fn run(cfg: &PipelineConfig) -> PipelineReport {
        let mut runner = ProcessCommandRunner;
        match PipelineLogger::new(cfg.log_dir.clone().into()) {
            Ok(mut logger) => Self::run_with(cfg, &mut runner, &mut logger),
            Err(error) => {
                let mut sink = FailedEventSink(error.to_string());
                Self::run_with(cfg, &mut runner, &mut sink)
            }
        }
    }

    /// Run the legacy check-only contract.
    ///
    /// Checks run in build, lint, tests, and typecheck order. The benchmark
    /// toggle is intentionally ignored for compatibility. The report always
    /// has empty `tools` and no reward, and this method does not create logs or
    /// probe tools.
    ///
    /// # Example
    ///
    /// ```
    /// use looprs::app_config::PipelineChecksConfig;
    /// use looprs::pipeline::PipelineRunner;
    ///
    /// let report = PipelineRunner::run_checks(&PipelineChecksConfig::default());
    /// assert!(report.steps.is_empty());
    /// assert!(report.tools.is_empty());
    /// assert!(report.reward.is_none());
    /// ```
    pub fn run_checks(cfg: &PipelineChecksConfig) -> PipelineReport {
        let mut runner = ProcessCommandRunner;
        Self::run_checks_with(cfg, &mut runner)
    }

    /// A failed pipeline blocks the agent only when `block_on_failure` is set.
    pub fn should_block(cfg: &PipelineConfig, report: &PipelineReport) -> bool {
        cfg.block_on_failure && !report.succeeds(cfg.reward_threshold)
    }

    /// Run the expanded pipeline through caller-provided command and event ports.
    ///
    /// This entry point provides deterministic integration without changing
    /// the behavior of [`Self::run_checks`].
    ///
    /// # Example
    ///
    /// ```
    /// use looprs::app_config::PipelineConfig;
    /// use looprs::pipeline::{PipelineCommandRunner, PipelineEventSink, PipelineRunner};
    ///
    /// struct Pass;
    /// impl PipelineCommandRunner for Pass {
    ///     fn run(&mut self, _program: &str, _args: &[&str]) -> std::io::Result<bool> {
    ///         Ok(true)
    ///     }
    /// }
    /// struct Ignore;
    /// impl PipelineEventSink for Ignore {
    ///     fn record(&mut self, _step: &str, _data: serde_json::Value) -> std::io::Result<()> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let report = PipelineRunner::run_with(&PipelineConfig::default(), &mut Pass, &mut Ignore);
    /// assert!(report.steps.is_empty());
    /// ```
    pub fn run_with<R, S>(cfg: &PipelineConfig, runner: &mut R, sink: &mut S) -> PipelineReport
    where
        R: PipelineCommandRunner,
        S: PipelineEventSink,
    {
        let mut steps = Vec::new();
        let mut tools = Vec::new();
        let mut logging_failed = false;
        let checks_enabled = cfg.checks.run_build
            || cfg.checks.run_tests
            || cfg.checks.run_lint
            || cfg.checks.run_typecheck
            || cfg.checks.run_bench;

        let cargo_available = !checks_enabled || tool_available(runner, "cargo");
        if checks_enabled {
            tools.push(Self::tool_result(
                "cargo",
                cargo_available,
                cfg.require_tools,
            ));
        }

        let nextest_available = cfg.checks.run_tests && tool_available(runner, "cargo-nextest");
        if cfg.checks.run_tests {
            tools.push(Self::tool_result("cargo-nextest", nextest_available, false));
        }

        if cfg.require_tools && !cargo_available {
            Self::record_step(
                &mut steps,
                sink,
                &mut logging_failed,
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
                let success = if name == "tests" && nextest_available {
                    run_nextest_with_fallback(runner)
                } else {
                    runner.run("cargo", &args).unwrap_or(false)
                };
                let result = StepResult {
                    step: name.into(),
                    success,
                };
                let failed = !result.success;
                Self::record_step(&mut steps, sink, &mut logging_failed, result);
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
        let threshold = normalize_reward_threshold(cfg.reward_threshold);
        if reward < threshold {
            Self::record_step(
                &mut steps,
                sink,
                &mut logging_failed,
                StepResult {
                    step: "reward_threshold".into(),
                    success: false,
                },
            );
        }

        let mut report = PipelineReport {
            steps,
            tools,
            reward: Some(RewardReport { reward }),
        };
        let data = serde_json::to_value(&report)
            .unwrap_or_else(|error| serde_json::json!({"error": error.to_string()}));
        if sink.record("pipeline_complete", data).is_err() {
            logging_failed = true;
        }
        if logging_failed {
            report.steps.push(StepResult {
                step: "logging".into(),
                success: false,
            });
        }
        report
    }

    fn run_checks_with<R>(cfg: &PipelineChecksConfig, runner: &mut R) -> PipelineReport
    where
        R: PipelineCommandRunner,
    {
        let mut steps = Vec::new();
        let configured_steps = [
            (
                cfg.run_build,
                "build",
                &["build", "--workspace", "--quiet"][..],
            ),
            (
                cfg.run_lint,
                "lint",
                &["clippy", "--workspace", "--quiet", "--", "-D", "warnings"][..],
            ),
        ];
        for (enabled, step, args) in configured_steps {
            if enabled {
                steps.push(StepResult {
                    step: step.into(),
                    success: runner.run("cargo", args).unwrap_or(false),
                });
            }
        }
        if cfg.run_tests {
            steps.push(StepResult {
                step: "tests".into(),
                success: run_nextest_with_fallback(runner),
            });
        }
        if cfg.run_typecheck {
            steps.push(StepResult {
                step: "typecheck".into(),
                success: runner
                    .run("cargo", &["check", "--workspace", "--quiet"])
                    .unwrap_or(false),
            });
        }
        PipelineReport {
            steps,
            tools: Vec::new(),
            reward: None,
        }
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
        sink: &mut impl PipelineEventSink,
        logging_failed: &mut bool,
        result: StepResult,
    ) {
        if sink
            .record(&result.step, serde_json::json!({"success": result.success}))
            .is_err()
        {
            *logging_failed = true;
        }
        steps.push(result);
    }
}

fn run_nextest_with_fallback(runner: &mut impl PipelineCommandRunner) -> bool {
    runner
        .run("cargo", &["nextest", "run", "--workspace", "--quiet"])
        .or_else(|_| runner.run("cargo", &["test", "--workspace", "--quiet"]))
        .unwrap_or(false)
}

fn tool_available(runner: &mut impl PipelineCommandRunner, tool: &str) -> bool {
    runner.run(tool, &["--version"]).unwrap_or(false)
}

pub(crate) fn normalize_reward_threshold(threshold: f32) -> f64 {
    if threshold.is_nan() {
        0.0
    } else {
        f64::from(threshold.clamp(0.0, 1.0))
    }
}

struct FailedEventSink(String);

impl PipelineEventSink for FailedEventSink {
    fn record(&mut self, _step: &str, _data: serde_json::Value) -> io::Result<()> {
        Err(io::Error::other(self.0.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::PipelineConfig;
    use std::collections::{HashMap, VecDeque};
    use std::io;

    #[derive(Default)]
    struct FakeCommandRunner {
        calls: Vec<(String, Vec<String>)>,
        outcomes: HashMap<String, VecDeque<io::Result<bool>>>,
    }

    impl FakeCommandRunner {
        fn respond(&mut self, program: &str, args: &[&str], outcome: io::Result<bool>) {
            self.outcomes
                .entry(command_key(program, args))
                .or_default()
                .push_back(outcome);
        }
    }

    impl PipelineCommandRunner for FakeCommandRunner {
        fn run(&mut self, program: &str, args: &[&str]) -> io::Result<bool> {
            self.calls.push((
                program.to_string(),
                args.iter().map(|arg| (*arg).to_string()).collect(),
            ));
            self.outcomes
                .get_mut(&command_key(program, args))
                .and_then(VecDeque::pop_front)
                .unwrap_or(Ok(true))
        }
    }

    #[derive(Default)]
    struct FakeEventSink {
        events: Vec<String>,
        fail: bool,
    }

    impl PipelineEventSink for FakeEventSink {
        fn record(&mut self, step: &str, _data: serde_json::Value) -> io::Result<()> {
            self.events.push(step.to_string());
            if self.fail {
                Err(io::Error::other("event sink failed"))
            } else {
                Ok(())
            }
        }
    }

    fn command_key(program: &str, args: &[&str]) -> String {
        format!("{program} {}", args.join(" "))
    }

    fn all_disabled_checks() -> PipelineChecksConfig {
        PipelineChecksConfig {
            run_build: false,
            run_tests: false,
            run_lint: false,
            run_typecheck: false,
            run_bench: false,
        }
    }

    fn config() -> PipelineConfig {
        PipelineConfig {
            checks: all_disabled_checks(),
            ..PipelineConfig::default()
        }
    }

    #[test]
    fn public_run_checks_preserves_empty_legacy_report() {
        let report = PipelineRunner::run_checks(&all_disabled_checks());
        assert!(report.steps.is_empty());
        assert!(report.tools.is_empty());
        assert!(report.reward.is_none());
    }

    #[test]
    fn legacy_run_checks_keeps_order_output_and_ignores_bench() {
        let mut checks = all_disabled_checks();
        checks.run_build = true;
        checks.run_lint = true;
        checks.run_tests = true;
        checks.run_typecheck = true;
        checks.run_bench = true;
        let mut runner = FakeCommandRunner::default();

        let report = PipelineRunner::run_checks_with(&checks, &mut runner);

        assert_eq!(
            report
                .steps
                .iter()
                .map(|step| step.step.as_str())
                .collect::<Vec<_>>(),
            ["build", "lint", "tests", "typecheck"]
        );
        assert!(report.tools.is_empty());
        assert!(report.reward.is_none());
        assert_eq!(runner.calls.len(), 4);
    }

    #[test]
    fn legacy_nextest_spawn_failure_falls_back_to_cargo_test() {
        let mut checks = all_disabled_checks();
        checks.run_tests = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond(
            "cargo",
            &["nextest", "run", "--workspace", "--quiet"],
            Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
        );
        runner.respond("cargo", &["test", "--workspace", "--quiet"], Ok(true));

        let report = PipelineRunner::run_checks_with(&checks, &mut runner);

        assert!(report.steps[0].success);
        assert_eq!(runner.calls.len(), 2);
    }

    #[test]
    fn public_run_returns_expanded_empty_report_and_writes_completion_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config();
        cfg.log_dir = dir.path().to_string_lossy().into_owned();

        let report = PipelineRunner::run(&cfg);

        assert!(report.steps.is_empty());
        assert_eq!(report.reward.unwrap().reward, 1.0);
        let entries = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(entries.contains("\"step\":\"pipeline_complete\""));
    }

    #[test]
    fn expanded_run_uses_nextest_fallback_after_spawn_failure() {
        let mut cfg = config();
        cfg.checks.run_tests = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        runner.respond("cargo-nextest", &["--version"], Ok(true));
        runner.respond(
            "cargo",
            &["nextest", "run", "--workspace", "--quiet"],
            Err(io::Error::new(io::ErrorKind::NotFound, "spawn failed")),
        );
        runner.respond("cargo", &["test", "--workspace", "--quiet"], Ok(true));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert!(report.steps.iter().all(|step| step.success));
        assert!(
            runner
                .calls
                .iter()
                .any(|(_, args)| args.first().is_some_and(|arg| arg == "test"))
        );
    }

    #[test]
    fn expanded_run_keeps_configured_order_and_benchmark() {
        let mut cfg = config();
        cfg.checks.run_build = true;
        cfg.checks.run_tests = true;
        cfg.checks.run_lint = true;
        cfg.checks.run_typecheck = true;
        cfg.checks.run_bench = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        runner.respond("cargo-nextest", &["--version"], Ok(false));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(
            report
                .steps
                .iter()
                .map(|step| step.step.as_str())
                .collect::<Vec<_>>(),
            ["build", "tests", "lint", "typecheck", "bench"]
        );
    }

    #[test]
    fn expanded_fail_fast_stops_after_first_failed_check() {
        let mut cfg = config();
        cfg.fail_fast = true;
        cfg.checks.run_build = true;
        cfg.checks.run_lint = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        runner.respond("cargo", &["build", "--workspace", "--quiet"], Ok(false));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "build");
    }

    #[test]
    fn command_spawn_failure_is_a_failed_check() {
        let mut cfg = config();
        cfg.checks.run_build = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        runner.respond(
            "cargo",
            &["build", "--workspace", "--quiet"],
            Err(io::Error::other("spawn failed")),
        );
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.steps[0].step, "build");
        assert!(!report.steps[0].success);
    }

    #[test]
    fn logging_failure_is_reported_without_stopping_checks() {
        let mut cfg = config();
        cfg.checks.run_build = true;
        cfg.checks.run_lint = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        let mut sink = FakeEventSink {
            fail: true,
            ..FakeEventSink::default()
        };

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.steps[0].step, "build");
        assert_eq!(report.steps[1].step, "lint");
        assert_eq!(report.steps.last().unwrap().step, "logging");
        assert!(!report.steps.last().unwrap().success);
    }

    #[test]
    fn empty_expanded_run_has_full_report_without_tool_calls() {
        let cfg = config();
        let mut runner = FakeCommandRunner::default();
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert!(report.steps.is_empty());
        assert!(report.tools.is_empty());
        assert_eq!(report.reward.unwrap().reward, 1.0);
        assert!(runner.calls.is_empty());
        assert_eq!(sink.events, ["pipeline_complete"]);
    }

    #[test]
    fn reward_threshold_is_normalized_and_equality_passes() {
        for (threshold, threshold_failure) in
            [(f32::NAN, false), (-1.0, false), (0.5, false), (2.0, true)]
        {
            let mut cfg = config();
            cfg.reward_threshold = threshold;
            cfg.checks.run_build = true;
            cfg.checks.run_lint = true;
            let mut runner = FakeCommandRunner::default();
            runner.respond("cargo", &["--version"], Ok(true));
            runner.respond("cargo", &["build", "--workspace", "--quiet"], Ok(true));
            runner.respond(
                "cargo",
                &["clippy", "--workspace", "--quiet", "--", "-D", "warnings"],
                Ok(false),
            );
            let mut sink = FakeEventSink::default();

            let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);
            let has_threshold_failure = report
                .steps
                .iter()
                .any(|step| step.step == "reward_threshold");

            assert_eq!(has_threshold_failure, threshold_failure, "{threshold}");
        }
    }

    #[test]
    fn required_tool_failure_does_not_count_as_attempted_check() {
        let mut cfg = config();
        cfg.require_tools = true;
        cfg.reward_threshold = 0.5;
        cfg.checks.run_build = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(false));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.reward.unwrap().reward, 0.0);
        assert_eq!(report.steps[0].step, "required_tool:cargo");
        assert_eq!(report.steps[1].step, "reward_threshold");
    }

    #[test]
    fn required_cargo_failure_is_the_only_preflight_failure() {
        let mut cfg = config();
        cfg.require_tools = true;
        cfg.checks.run_build = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(false));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "required_tool:cargo");
        assert!(!report.steps[0].success);
    }

    #[test]
    fn unavailable_optional_nextest_is_not_a_required_tool_failure() {
        let mut cfg = config();
        cfg.require_tools = true;
        cfg.checks.run_tests = true;
        let mut runner = FakeCommandRunner::default();
        runner.respond("cargo", &["--version"], Ok(true));
        runner.respond("cargo-nextest", &["--version"], Ok(false));
        runner.respond("cargo", &["test", "--workspace", "--quiet"], Ok(true));
        let mut sink = FakeEventSink::default();

        let report = PipelineRunner::run_with(&cfg, &mut runner, &mut sink);

        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "tests");
        assert!(report.steps[0].success);
    }

    #[test]
    fn public_run_reports_logger_initialization_failure() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        std::fs::write(&file, "occupied").unwrap();
        let mut cfg = config();
        cfg.log_dir = file.join("logs").to_string_lossy().into_owned();

        let report = PipelineRunner::run(&cfg);

        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].step, "logging");
        assert!(!report.steps[0].success);
    }

    #[test]
    fn failure_policy_only_blocks_when_configured() {
        let mut cfg = config();
        let report = PipelineReport {
            steps: vec![StepResult {
                step: "build".into(),
                success: false,
            }],
            tools: vec![],
            reward: Some(RewardReport { reward: 0.0 }),
        };

        assert!(!PipelineRunner::should_block(&cfg, &report));
        cfg.block_on_failure = true;
        assert!(PipelineRunner::should_block(&cfg, &report));
    }

    #[test]
    fn failure_policy_applies_normalized_reward_threshold() {
        let mut cfg = config();
        cfg.block_on_failure = true;
        cfg.reward_threshold = 2.0;
        let report = PipelineReport {
            steps: vec![],
            tools: vec![],
            reward: Some(RewardReport { reward: 0.5 }),
        };

        assert!(PipelineRunner::should_block(&cfg, &report));
        cfg.reward_threshold = f32::NAN;
        assert!(!PipelineRunner::should_block(&cfg, &report));
    }
}
