use super::*;
use looprs_desktop::ui::editor::editor_screen;

#[test]
fn test_editor_renders_default_text() {
    let mut test = test_runner(editor_screen());
    test.sync_and_update();

    assert_text_contains(&test, "looprs desktop editor");
    assert_text_contains(&test, "Edit me with freya-code-editor");
}

#[test]
fn test_editor_has_scratchpad_label() {
    let mut test = test_runner(editor_screen());
    test.sync_and_update();

    assert_text_contains(&test, "Scratchpad");
}

#[test]
fn test_editor_background_color() {
    let mut test = test_runner(editor_screen());
    test.sync_and_update();

    // Snapshot test for visual regression
    insta::assert_snapshot!(test.get_root_text());
}
