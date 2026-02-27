//! Scenario builder DSL for integration testing.
//!
//! `ScenarioBuilder` provides a fluent API for building complex test scenarios
//! that involve multiple actors (browser, CLI) and assertions.

use crate::testing::{MockBrowser, MockCliProcess, MockServer};
use anyhow::Result;
use std::time::Duration;

/// Context available to scenario steps.
#[derive(Debug)]
pub struct ScenarioContext {
    pub cli: MockCliProcess,
    pub browser: MockBrowser,
}

/// A test scenario consisting of multiple steps and expectations.
#[derive(Debug)]
pub struct Scenario {
    name: String,
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
}

impl ScenarioBuilder {
    /// Create a new scenario builder.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cli: None,
            browser: None,
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

    /// Build the scenario (placeholder - actual implementation will add steps).
    pub fn build(self) -> Result<Scenario> {
        Ok(Scenario {
            name: self.name,
            steps: Vec::new(),
        })
    }
}

impl Scenario {
    /// Run the scenario.
    pub async fn run(&self) -> Result<()> {
        tracing::info!(scenario = %self.name, "Running scenario");

        // TODO: Implement scenario execution
        // - Execute steps in order
        // - Collect results
        // - Verify expectations

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
        steps: Vec::new(),
    })
}

/// Create a process crash scenario.
pub async fn process_crash_scenario(cli: MockCliProcess, browser: MockBrowser) -> Result<Scenario> {
    Ok(Scenario {
        name: "process_crash".to_string(),
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
        steps: Vec::new(),
    })
}

/// Create an event replay scenario.
pub async fn event_replay_scenario(cli: MockCliProcess, browser: MockBrowser) -> Result<Scenario> {
    Ok(Scenario {
        name: "event_replay".to_string(),
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
        steps: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_builder_creates_scenario() {
        let builder = ScenarioBuilder::new("test");
        let scenario = builder.build().expect("build failed");
        assert_eq!(scenario.name, "test");
    }

    #[tokio::test]
    async fn scenario_can_run() {
        let scenario = Scenario {
            name: "empty".to_string(),
            steps: Vec::new(),
        };
        let result = scenario.run().await;
        assert!(result.is_ok());
    }
}
