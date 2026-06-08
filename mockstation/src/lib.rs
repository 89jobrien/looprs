//! Mockstation integration test harness prototype.
//!
//! This module provides a scenario-builder DSL for testing browser-to-CLI
//! interactions in the looprs system.

pub mod helpers;
pub mod ui {
    pub mod mock_browser;
    pub mod mock_cli;
    // Note: mock_server and mock_ws require maestro_companion which is not available
    // in the looprs workspace yet. They are included in the prototype structure
    // but not compiled until integration is complete.
}

/// Re-export testing types for convenient access in scenarios.
pub mod testing {
    pub use crate::ui::mock_browser::MockBrowser;
    pub use crate::ui::mock_cli::MockCliProcess;

    // Mock server is not yet available due to external dependencies
    #[derive(Debug, Clone)]
    pub struct MockServer;
}
