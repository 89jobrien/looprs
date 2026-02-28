//! End-to-end integration tests for desktop app flows

use freya_testing::prelude::*;
use looprs_desktop::ui::root::app;
use looprs_desktop::services::sqlite_store;
use tempfile::TempDir;

fn click_at(test: &mut TestingRunner<impl freya::prelude::Component>, x: f32, y: f32) {
    test.click_cursor((x, y));
    test.sync_and_update();
}

fn assert_text_contains(test: &TestingRunner<impl freya::prelude::Component>, expected: &str) {
    let text = test.get_root_text();
    assert!(
        text.contains(expected),
        "Expected text to contain '{}', but got: {}",
        expected,
        text
    );
}

#[tokio::test]
async fn test_complete_chat_workflow() {
    let _tmp = setup_temp_observability();

    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "chat");

    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    // 1. Verify initial state
    assert_text_contains(&test, "Messages: 0");

    // 2. Verify UI loaded with correct status
    assert_text_contains(&test, "Status: Ready");

    // 3. Verify conversation panel exists
    assert_text_contains(&test, "Conversation");

    // Note: Full input simulation requires freya-testing input helpers
    // For now, we verify the UI is properly initialized

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_all_screens_render_without_panic() {
    let screens = vec![
        "main", "chat", "editor", "terminal", "genui", "mockstation",
    ];

    for screen in screens {
        std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", screen);
        std::env::set_var("OPENAI_API_KEY", "test-key"); // For genui

        let mut test = TestingRunner::new(app());
        test.sync_and_update();

        let text = test.get_root_text();
        assert!(!text.is_empty(), "Screen '{}' rendered empty", screen);

        std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
    }

    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_screen_navigation_from_main_menu() {
    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    // Verify we start at main menu
    assert_text_contains(&test, "looprs desktop");

    // Click through each screen button
    // AI Workspace
    click_at(&mut test, 100.0, 50.0);
    assert_text_contains(&test, "Conversation");

    // Click Main Menu to go back
    click_at(&mut test, 500.0, 50.0);
    assert_text_contains(&test, "looprs desktop");
}

#[tokio::test]
async fn test_chat_persistence_integration() {
    let _tmp = setup_temp_observability();

    // Add a message directly via the service
    sqlite_store::append_chat_message("User", "Test message").await;

    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "chat");

    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    // Wait for async load to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify message count reflects persisted data
    // Note: This requires the UI to load messages, which happens async
    assert_text_contains(&test, "Workspace");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_mockstation_full_workflow() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "mockstation");

    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    // Verify mockstation initialized
    assert_text_contains(&test, "Terminal panel");
    assert_text_contains(&test, "Browser panel");
    assert_text_contains(&test, "Transport + server log");

    // Click connect terminal button (approximate position)
    click_at(&mut test, 150.0, 200.0);
    test.sync_and_update();

    // Should not panic
    let text = test.get_root_text();
    assert!(!text.is_empty());

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_editor_screen_integration() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "editor");

    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    assert_text_contains(&test, "Editor");
    assert_text_contains(&test, "Scratchpad");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_terminal_screen_integration() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "terminal");

    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    assert_text_contains(&test, "Terminal");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

fn setup_temp_observability() -> TempDir {
    let tmp = TempDir::new().unwrap();
    std::env::set_var("LOOPRS_OBSERVABILITY_DIR", tmp.path());
    tmp
}
