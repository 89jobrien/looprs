//! Headless UI component tests using freya-testing
//!
//! Tests in this module use freya's headless renderer to test UI components
//! without requiring a window or graphics context.

use freya::prelude::*;
use freya_testing::prelude::*;

pub mod root_tests;
pub mod editor_tests;
pub mod terminal_tests;

/// Helper to create a test runner with common setup
pub fn test_runner<T: Component + 'static>(component: T) -> TestingRunner<T> {
    TestingRunner::new(component)
}

/// Helper to click at specific coordinates
pub fn click_at(test: &mut TestingRunner<impl Component>, x: f32, y: f32) {
    test.click_cursor((x, y));
    test.sync_and_update();
}

/// Helper to verify text content exists
pub fn assert_text_contains(test: &TestingRunner<impl Component>, expected: &str) {
    let text = test.get_root_text();
    assert!(
        text.contains(expected),
        "Expected text to contain '{}', but got: {}",
        expected,
        text
    );
}
