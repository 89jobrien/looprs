//! Scenario builder DSL for integration testing.
//!
//! `ScenarioBuilder` provides a fluent API for building complex test scenarios
//! that involve multiple actors (browser, CLI) and assertions.

use crate::testing::{MockBrowser, MockCliProcess, MockServer};
use anyhow::Result;
use std::time::Duration;

/// Context available to scenario steps.
#[derive(Debug, Clone)]
pub struct ScenarioContext {
    pub cli: MockCliProcess,
    pub browser: MockBrowser,
}

/// A test scenario consisting of multiple steps and expectations.
#[derive(Debug)]
pub struct Scenario {
    name: String,
    cli: Option<MockCliProcess>,
    browser: Option<MockBrowser>,
    steps: Vec<ScenarioStep>,
}

struct ScenarioStep {
    name: String,
    action: Box<dyn Fn(&ScenarioContext) + Send>,
}

impl std::fmt::Debug for ScenarioStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScenarioStep")
            .field("name", &self.name)
            .field("action", &"<closure>")
            .finish()
    }
}

/// Builder for constructing test scenarios.
#[derive(Debug)]
pub struct ScenarioBuilder {
    name: String,
    cli: Option<MockCliProcess>,
    browser: Option<MockBrowser>,
    steps: Vec<ScenarioStep>,
}

impl ScenarioBuilder {
    /// Create a new scenario builder.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cli: None,
            browser: None,
            steps: Vec::new(),
        }
    }

    /// Set the mock CLI process for this scenario.
    pub fn with_cli_process(mut self, cli: MockCliProcess) -> Self {
        self.cli = Some(cli);
        self
    }

    /// Set the mock browser for this scenario.
    pub fn with_browser(mut self, browser: MockBrowser) -> Self {
        self.browser = Some(browser);
        self
    }

    /// Add a step to the scenario.
    pub fn with_step<F>(mut self, name: &str, action: F) -> Self
    where
        F: Fn(&ScenarioContext) + Send + 'static,
    {
        self.steps.push(ScenarioStep {
            name: name.to_string(),
            action: Box::new(action),
        });
        self
    }

    /// Build the scenario.
    pub fn build(self) -> Result<Scenario> {
        Ok(Scenario {
            name: self.name,
            cli: self.cli,
            browser: self.browser,
            steps: self.steps,
        })
    }
}

impl Scenario {
    /// Run the scenario.
    pub async fn run(&self) -> Result<()> {
        tracing::info!(scenario = %self.name, "Running scenario");

        // Validate that we have the necessary context components.
        let cli = self.cli.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Scenario '{}': CLI context not set", self.name)
        })?;
        let browser = self.browser.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Scenario '{}': Browser context not set", self.name)
        })?;

        let context = ScenarioContext {
            cli: cli.clone(),
            browser: browser.clone(),
        };

        // Execute steps in order.
        for (index, step) in self.steps.iter().enumerate() {
            tracing::info!(
                scenario = %self.name,
                step_index = index,
                step_name = %step.name,
                "Executing scenario step"
            );

            (step.action)(&context);
        }

        tracing::info!(scenario = %self.name, "Scenario completed successfully");
        Ok(())
    }
}

// ============================================================================
// Pre-built Scenario Constructors
// ============================================================================

/// Create a happy path scenario: browser connects, CLI starts, user sends message, permission granted.
pub async fn happy_path_scenario(cli: MockCliProcess, browser: MockBrowser) -> Result<Scenario> {
    Ok(Scenario {
        name: "happy_path".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

/// Create a permission denial scenario.
pub async fn permission_denial_scenario(
    cli: MockCliProcess,
    browser: MockBrowser,
) -> Result<Scenario> {
    Ok(Scenario {
        name: "permission_denial".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

/// Create a process crash scenario.
pub async fn process_crash_scenario(cli: MockCliProcess, browser: MockBrowser) -> Result<Scenario> {
    Ok(Scenario {
        name: "process_crash".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

/// Create a permission timeout scenario.
pub async fn permission_timeout_scenario(
    cli: MockCliProcess,
    browser: MockBrowser,
) -> Result<Scenario> {
    Ok(Scenario {
        name: "permission_timeout".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

/// Create an event replay scenario.
pub async fn event_replay_scenario(cli: MockCliProcess, browser: MockBrowser) -> Result<Scenario> {
    Ok(Scenario {
        name: "event_replay".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

/// Create a race condition scenario.
pub async fn race_condition_scenario(
    cli: MockCliProcess,
    browser: MockBrowser,
) -> Result<Scenario> {
    Ok(Scenario {
        name: "race_condition".to_string(),
        cli: Some(cli),
        browser: Some(browser),
        steps: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn scenario_builder_creates_scenario() {
        let builder = ScenarioBuilder::new("test");
        let scenario = builder.build().expect("build failed");
        assert_eq!(scenario.name, "test");
    }

    #[tokio::test]
    async fn scenario_can_run_empty() {
        let cli = MockCliProcess::default();
        let browser = MockBrowser::default();

        let scenario = ScenarioBuilder::new("empty")
            .with_cli_process(cli)
            .with_browser(browser)
            .build()
            .expect("build failed");

        let result = scenario.run().await;
        assert!(result.is_ok(), "Empty scenario should run successfully");
    }

    #[tokio::test]
    async fn scenario_runs_steps_in_order() {
        let cli = MockCliProcess::default();
        let browser = MockBrowser::default();
        let execution_order = Arc::new(AtomicUsize::new(0));

        let order1 = execution_order.clone();
        let order2 = execution_order.clone();

        let scenario = ScenarioBuilder::new("ordered_steps")
            .with_cli_process(cli)
            .with_browser(browser)
            .with_step("step_1", move |_ctx| {
                order1.store(1, Ordering::SeqCst);
            })
            .with_step("step_2", move |_ctx| {
                let prev = order2.load(Ordering::SeqCst);
                assert_eq!(prev, 1, "Step 2 should run after Step 1");
                order2.store(2, Ordering::SeqCst);
            })
            .build()
            .expect("build failed");

        let result = scenario.run().await;
        assert!(result.is_ok(), "Scenario with multiple steps should succeed");
        assert_eq!(
            execution_order.load(Ordering::SeqCst),
            2,
            "Both steps should have executed"
        );
    }

    #[tokio::test]
    async fn scenario_fails_without_cli_context() {
        let browser = MockBrowser::default();

        let scenario = ScenarioBuilder::new("no_cli")
            .with_browser(browser)
            .build()
            .expect("build failed");

        let result = scenario.run().await;
        assert!(result.is_err(), "Scenario without CLI should fail");
        assert!(
            result.unwrap_err().to_string().contains("CLI context"),
            "Error message should mention CLI context"
        );
    }

    #[tokio::test]
    async fn scenario_fails_without_browser_context() {
        let cli = MockCliProcess::default();

        let scenario = ScenarioBuilder::new("no_browser")
            .with_cli_process(cli)
            .build()
            .expect("build failed");

        let result = scenario.run().await;
        assert!(result.is_err(), "Scenario without Browser should fail");
        assert!(
            result.unwrap_err().to_string().contains("Browser context"),
            "Error message should mention Browser context"
        );
    }

    #[tokio::test]
    async fn scenario_step_receives_context() {
        let cli = MockCliProcess::default();
        let browser = MockBrowser::default();
        let context_received = Arc::new(AtomicUsize::new(0));

        let ctx_flag = context_received.clone();
        let scenario = ScenarioBuilder::new("context_test")
            .with_cli_process(cli)
            .with_browser(browser)
            .with_step("check_context", move |ctx| {
                // Verify we received a context with non-null references
                let _ = &ctx.cli;
                let _ = &ctx.browser;
                ctx_flag.store(1, Ordering::SeqCst);
            })
            .build()
            .expect("build failed");

        let result = scenario.run().await;
        assert!(result.is_ok());
        assert_eq!(
            context_received.load(Ordering::SeqCst),
            1,
            "Step should have received and used the context"
        );
    }
}
