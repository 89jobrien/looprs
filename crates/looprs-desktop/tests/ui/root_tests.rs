use super::*;
use looprs_desktop::ui::root::app;

#[test]
fn test_initial_screen_is_main_menu() {
    let mut test = test_runner(app());
    test.sync_and_update();

    assert_text_contains(&test, "looprs desktop");
    assert_text_contains(&test, "Main Menu");
}

#[test]
fn test_navigation_to_ai_chat_screen() {
    let mut test = test_runner(app());
    test.sync_and_update();

    // Click AI Workspace button
    click_at(&mut test, 100.0, 50.0);

    assert_text_contains(&test, "Ask looprs anything");
    assert_text_contains(&test, "Conversation");
}

#[test]
fn test_chat_input_and_status() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "chat");

    let mut test = test_runner(app());
    test.sync_and_update();

    // Verify initial state
    assert_text_contains(&test, "Status: Ready");
    assert_text_contains(&test, "Messages: 0");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_generative_ui_lifecycle() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "genui");
    std::env::set_var("OPENAI_API_KEY", "test-key");

    let mut test = test_runner(app());
    test.sync_and_update();

    // Verify GenUI screen loaded
    assert_text_contains(&test, "Live Generative UI");
    assert_text_contains(&test, "Status:");

    // Navigate away
    click_at(&mut test, 500.0, 50.0); // Click Main Menu
    test.sync_and_update();

    // Verify cleanup (handle stopped)
    assert_text_contains(&test, "Main Menu");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
fn test_mockstation_initialization() {
    std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", "mockstation");

    let mut test = test_runner(app());
    test.sync_and_update();

    assert_text_contains(&test, "Terminal panel");
    assert_text_contains(&test, "Browser panel");
    assert_text_contains(&test, "Transport + server log");

    std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
}

#[test]
fn test_screen_enum_values() {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn screen_env_var_parsing(screen in prop_oneof![
            Just("main"),
            Just("chat"),
            Just("editor"),
            Just("terminal"),
            Just("genui"),
            Just("mockstation"),
        ]) {
            std::env::set_var("LOOPRS_DESKTOP_START_SCREEN", screen);

            let mut test = test_runner(app());
            test.sync_and_update();

            // Should not panic and should render something
            let text = test.get_root_text();
            assert!(!text.is_empty());

            std::env::remove_var("LOOPRS_DESKTOP_START_SCREEN");
        }
    }
}
