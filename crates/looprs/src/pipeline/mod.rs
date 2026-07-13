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
            steps.push(Self::run_step("build", &["build", "--workspace", "--quiet"]));
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
            steps.push(Self::run_step("typecheck", &["check", "--workspace", "--quiet"]));
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
}
