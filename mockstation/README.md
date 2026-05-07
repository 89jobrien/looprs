# Mockstation prototype

This directory contains tracked prototype scaffolding for a richer Mockstation integration-test harness. It is not currently an independent Rust crate and is not wired into the `looprs` workspace build.

## Purpose

The prototype appears intended to model a browser-to-CLI companion session for testing:

- browser JSON protocol messages
- CLI NDJSON/protocol messages
- permission request and response routing
- CLI process lifecycle control
- event sequencing and replay after browser reconnects
- mock browser, terminal, WebSocket client/server, and CLI process components
- scenario-style integration tests
  The active Mockstation feature used by the desktop app currently lives under `crates/looprs-desktop/src/services/`, especially `crates/looprs-desktop/src/services/mockstation.rs`. That implementation backs the Mockstation screen in `crates/looprs-desktop/src/ui/root.rs` and is covered by `crates/looprs-desktop/tests/mockstation_tests.rs`.

## Contents

- `src/helpers/scenario_builder.rs` defines a scenario-builder DSL and named scenario constructors. Most scenarios are currently skeletons with empty step lists.
- `src/protocols/browser.rs` defines browser outbound and inbound message types, including user messages, permission responses, session replay, CLI lifecycle commands, assistant events, stream events, permission requests, and lifecycle events.
- `src/protocols/cli.rs` defines CLI message types, control requests, permission requests, and control responses.
- `src/sessions/bridge.rs` defines the intended central `SessionBridge` state machine for translating CLI messages to browser events and browser messages back to CLI messages.
- `src/sessions/buffer.rs` defines an event replay buffer for reconnect scenarios.
- `src/sessions/procman.rs` defines a process-manager abstraction for testable process lifecycle handling.
- `src/ui/mock_cli.rs` defines a mock CLI process with fake lifecycle state, injected events, fake PIDs, and message-log helpers.
- `src/ui/mock_server.rs` defines a mock WebSocket server handle with connection tracking and bridge access.
- `src/ui/mock_terminal.rs` defines a mock terminal that records output and captures input.
- `src/ui/mock_ws.rs` defines a browser WebSocket test client that can send user messages, permission responses, and replay subscriptions.
- `src/ui/mock_browser.rs` is currently empty.

## Current state

This directory is useful as design/reference material, but it is not ready to compile or run as part of `looprs`.
Observed issues:

- There is no `Cargo.toml`, `mod.rs`, or workspace member entry for this directory.
- Several files import `maestro_companion` and `maestro_test_ws`, which are not dependencies of the current `looprs` workspace.
- `src/sessions/buffer.rs` contains duplicate fields and duplicate method signatures from an apparent half-applied refactor.
- `src/sessions/bridge.rs` contains duplicate logic and duplicate struct fields in event emission paths.
- `src/helpers/scenario_builder.rs` is mostly stubbed and does not yet exercise real scenario steps.
- `src/ui/mock_browser.rs` is empty.

## Relationship to active desktop Mockstation

The current desktop Mockstation implementation is a simpler in-memory simulator. It supports:

- terminal connect and disconnect
- browser connect and disconnect
- WebSocket start and stop
- REST API start and stop
- simulated terminal commands
- browser REST calls
- bidirectional WebSocket message routing
- snapshots for terminal, browser, and transport/server log panels
  That implementation is active because it is part of the `looprs-desktop` crate. This prototype is inactive because no compiled crate or module references it.

## Decision options

There are two reasonable paths.

### Integrate the prototype

Choose this path if `looprs` needs a reusable, higher-fidelity test harness for protocol replay, CLI lifecycle, permission handling, and WebSocket/browser integration.
Expected direction:

- Create a real crate or module boundary for the prototype.
- Rename or adapt `maestro_*` references to `looprs` concepts, or add explicit dependencies if those crates are intentional.
- Repair compile errors in `buffer.rs` and `bridge.rs`.
- Add module exports and wire the harness into workspace tests.
- Migrate or compare behavior with the active desktop Mockstation service.
- Replace skeleton scenarios with executable integration tests.

### Archive the prototype

Choose this path if the active desktop Mockstation is sufficient and the prototype is not on the near-term roadmap.
Expected direction:

- Move the prototype to an archive location or remove it after confirming no active work depends on it.
- Keep this README or a short pointer in the archive explaining why it was archived.
- Preserve any useful protocol ideas in issues or design notes before removal.
- Keep the active desktop Mockstation service as the supported implementation.

## Recommendation

Archive first unless there is a concrete near-term need for protocol-level browser/CLI integration testing. The prototype contains useful architectural ideas, but its current compile state and external `maestro_*` references mean integration would be a non-trivial project rather than a cleanup pass.
