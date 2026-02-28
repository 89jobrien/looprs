use looprs_desktop::services::mockstation::*;

#[test]
fn test_mockstation_initialization() {
    let runtime = build_mockstation_runtime();
    let snapshot = runtime.snapshot();

    assert!(snapshot.transport_log.contains("mockstation initialized"));
}

#[test]
fn test_terminal_connection_lifecycle() {
    let mut runtime = build_mockstation_runtime();

    runtime.connect_terminal();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("terminal connected"));

    runtime.disconnect_terminal();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("terminal disconnected"));
}

#[test]
fn test_browser_connection_lifecycle() {
    let mut runtime = build_mockstation_runtime();

    runtime.connect_browser();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("browser connected"));

    runtime.disconnect_browser();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("browser disconnected"));
}

#[test]
fn test_websocket_lifecycle() {
    let mut runtime = build_mockstation_runtime();

    runtime.start_websocket();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("websocket transport online"));

    runtime.stop_websocket();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("websocket transport offline"));
}

#[test]
fn test_rest_api_lifecycle() {
    let mut runtime = build_mockstation_runtime();

    runtime.start_rest_api();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("rest api server online"));

    runtime.stop_rest_api();
    let snapshot = runtime.snapshot();
    assert!(snapshot.transport_log.contains("rest api server offline"));
}

#[test]
fn test_websocket_message_routing() {
    let mut runtime = build_mockstation_runtime();

    runtime.connect_terminal();
    runtime.connect_browser();
    runtime.start_websocket();

    runtime.send_from_terminal("ping");
    let snapshot = runtime.snapshot();

    assert!(snapshot.browser_view.contains("ping"));
}

#[test]
fn test_rest_api_when_offline() {
    let mut runtime = build_mockstation_runtime();
    runtime.connect_browser();

    // REST API not started
    runtime.browser_rest_call("/api/health");

    let snapshot = runtime.snapshot();
    assert!(snapshot.browser_view.contains("503"));
}

#[test]
fn test_rest_api_when_online() {
    let mut runtime = build_mockstation_runtime();
    runtime.connect_browser();
    runtime.start_rest_api();

    runtime.browser_rest_call("/api/health");

    let snapshot = runtime.snapshot();
    assert!(snapshot.browser_view.contains("200 OK"));
}

#[test]
fn test_terminal_command_execution() {
    let mut runtime = build_mockstation_runtime();
    runtime.connect_terminal();

    runtime.run_terminal_command("ls -la");

    let snapshot = runtime.snapshot();
    assert!(snapshot.terminal_view.contains("$ ls -la"));
    assert!(snapshot.terminal_view.contains("command executed"));
}

#[test]
fn test_bidirectional_ws_messaging() {
    let mut runtime = build_mockstation_runtime();
    runtime.connect_terminal();
    runtime.connect_browser();
    runtime.start_websocket();

    // Terminal to browser
    runtime.send_from_terminal("ping from terminal");
    let snapshot = runtime.snapshot();
    assert!(snapshot.browser_view.contains("ping from terminal"));

    // Browser to terminal
    runtime.send_from_browser("pong from browser");
    let snapshot = runtime.snapshot();
    assert!(snapshot.terminal_view.contains("pong from browser"));
}

use proptest::prelude::*;

proptest! {
    #[test]
    fn test_snapshot_always_valid(
        terminal_connected in any::<bool>(),
        browser_connected in any::<bool>(),
        ws_running in any::<bool>(),
        rest_running in any::<bool>(),
    ) {
        let mut runtime = build_mockstation_runtime();

        if terminal_connected {
            runtime.connect_terminal();
        }
        if browser_connected {
            runtime.connect_browser();
        }
        if ws_running {
            runtime.start_websocket();
        }
        if rest_running {
            runtime.start_rest_api();
        }

        // Should never panic
        let snapshot = runtime.snapshot();

        assert!(!snapshot.transport_log.is_empty());
    }
}
