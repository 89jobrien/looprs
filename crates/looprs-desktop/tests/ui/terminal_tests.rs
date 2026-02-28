use super::*;
use looprs_desktop::ui::terminal::terminal_screen;

#[test]
fn test_terminal_initialization() {
    let mut test = test_runner(terminal_screen());
    test.sync_and_update();

    assert_text_contains(&test, "Terminal");
}

#[test]
fn test_terminal_renders_ui() {
    let mut test = test_runner(terminal_screen());
    test.sync_and_update();

    // Should render without panicking
    let text = test.get_root_text();
    assert!(!text.is_empty());
}

#[test]
fn test_terminal_handles_missing_bash() {
    // TODO: Mock CommandBuilder to test failure path
    // This requires adding a trait abstraction for terminal process spawning
}
