//! Mock browser for testing browser-to-CLI interactions.

use anyhow::Result;

/// A mock browser instance for testing scenarios.
#[derive(Debug, Clone)]
pub struct MockBrowser {
    // Placeholder for browser state
}

impl MockBrowser {
    /// Create a new mock browser.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for MockBrowser {
    fn default() -> Self {
        Self::new()
    }
}
