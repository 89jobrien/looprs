//! Conformance test suites for port traits.
//!
//! Each function asserts the semantic contract a trait promises. Call these
//! from any adapter's `#[cfg(test)]` module to prove the impl is correct.

// IDEA(M2): run assert_inference_provider_contract() against all 7 provider
// implementations (anthropic, openai, gemini, local, anthropic-sdk, openai-sdk, baml).
// The function skeleton exists in this file but only covers MessageBroker and SessionStore.
// Add a parallel suite for InferenceProvider (single-turn, tool-call, multi-turn).

use crate::ports::message_broker::{Message, MessageBroker};
use crate::ports::session_store::{SessionEvent, SessionStore};
use crate::ports::user_output::UserOutput;

// ── MessageBroker ───────────────────────────────────────────────────────

/// Assert that a `MessageBroker` implementation satisfies the full contract.
///
/// Contract:
/// 1. A subscriber receives messages published to its topic.
/// 2. Publishing with no subscribers returns 0.
/// 3. Multiple subscribers each receive the message (fan-out).
/// 4. Messages on different topics do not cross.
/// 5. After `close()`, publish returns 0.
pub fn assert_message_broker_contract(broker: impl MessageBroker + Clone) {
    // 1. Subscriber receives published message
    let mut rx = broker.subscribe("t1");
    let msg = Message::new("src", "t1", 1, serde_json::Value::Null);
    let n = broker.publish(msg);
    assert!(n >= 1, "expected at least 1 receiver, got {n}");
    let received = rx.try_recv().expect("subscriber should receive message");
    assert_eq!(received.topic, "t1");
    assert_eq!(received.source, "src");

    // 2. No subscribers returns 0
    let broker2 = broker.clone();
    let n = broker2.publish(Message::new("src", "no-sub", 1, serde_json::Value::Null));
    assert_eq!(n, 0, "publish with no subscribers should return 0");

    // 3. Fan-out to multiple subscribers
    let mut rx_a = broker.subscribe("fan");
    let mut rx_b = broker.subscribe("fan");
    let n = broker.publish(Message::new("src", "fan", 1, serde_json::Value::Null));
    assert_eq!(n, 2, "expected fan-out to 2 subscribers");
    assert!(rx_a.try_recv().is_ok());
    assert!(rx_b.try_recv().is_ok());

    // 4. Topic isolation
    let mut rx_x = broker.subscribe("x");
    let mut rx_y = broker.subscribe("y");
    broker.publish(Message::new("src", "x", 1, serde_json::Value::Null));
    assert!(rx_x.try_recv().is_ok(), "x subscriber should get x message");
    assert!(
        rx_y.try_recv().is_err(),
        "y subscriber should NOT get x message"
    );

    // 5. Close semantics
    broker.close();
    let n = broker.publish(Message::new("src", "t1", 1, serde_json::Value::Null));
    assert_eq!(n, 0, "publish after close should return 0");
}

// ── SessionStore ────────────────────────────────────────────────────────

/// Assert that a `SessionStore` implementation satisfies the full contract.
///
/// Contract:
/// 1. `session_id()` returns a stable, non-empty string.
/// 2. `log()` succeeds for every `SessionEvent` variant.
/// 3. `path()` returns a consistent value across calls.
pub fn assert_session_store_contract(store: &mut dyn SessionStore) {
    // 1. Stable, non-empty session id
    let id = store.session_id().to_string();
    assert!(!id.is_empty(), "session_id must not be empty");
    assert_eq!(
        store.session_id(),
        id,
        "session_id must be stable across calls"
    );

    // 2. Log every event variant without error
    let events = vec![
        SessionEvent::UserMessage {
            content: "hello".into(),
            provider: "test".into(),
        },
        SessionEvent::Inference {
            content: "response".into(),
            provider: "test".into(),
        },
        SessionEvent::ToolUse {
            tool_name: "bash".into(),
            input: serde_json::json!({"cmd": "echo"}),
            tool_use_id: "tu-1".into(),
            provider: "test".into(),
        },
        SessionEvent::ToolResult {
            tool_use_id: "tu-1".into(),
            output: "ok".into(),
            is_error: false,
            provider: "test".into(),
        },
        SessionEvent::SessionEnd,
    ];
    for (i, event) in events.into_iter().enumerate() {
        store
            .log(event)
            .unwrap_or_else(|e| panic!("log() failed on event variant {i}: {e}"));
    }

    // 3. path() is consistent
    let p1 = store.path().map(|p| p.to_path_buf());
    let p2 = store.path().map(|p| p.to_path_buf());
    assert_eq!(p1, p2, "path() must return consistent value");
}

// ── InferenceProvider ───────────────────────────────────────────────────

/// Assert that an `InferenceProvider` implementation satisfies the structural contract.
///
/// Contract:
/// 1. `name()` returns a non-empty string.
/// 2. `model()` returns a non-empty `ModelId`.
/// 3. `supports_tool_use()` returns without panic.
/// 4. `validate_config()` returns without panic (result is not asserted — providers
///    may legitimately return `Err` when env vars are absent in test context).
pub fn assert_inference_provider_contract(provider: &dyn crate::ports::InferenceProvider) {
    let name = provider.name();
    assert!(!name.is_empty(), "name() must return a non-empty string");

    let model = provider.model();
    assert!(
        !model.as_str().is_empty(),
        "model() must return a non-empty ModelId"
    );

    let _ = provider.supports_tool_use();
    let _ = provider.validate_config();
}

// ── UserOutput ──────────────────────────────────────────────────────────

/// Assert that a `UserOutput` implementation satisfies the full contract.
///
/// Contract: every method is callable without panic. This is a smoke-level
/// contract — the trait has no observable return values, so we verify
/// that the impl handles all inputs gracefully.
pub fn assert_user_output_contract(output: &dyn UserOutput) {
    output.info("info message");
    output.info("");
    output.warn("warning message");
    output.warn("");
    output.error("error message");
    output.error("");
    output.assistant_text("assistant text");
    output.assistant_text("");
    output.tool_call("bash", "echo hello");
    output.tool_call("", "");
    output.tool_ok();
    output.tool_err("something failed");
    output.tool_err("");
    output.write_chunk("chunk");
    output.write_chunk("");
}

// TODO(conformance): expand InferenceProvider contract test suite (idea #6).
// The current `assert_inference_provider_contract` only checks structural
// invariants (non-empty name, non-empty model). Add a live test matrix gated on
// LOOPRS_RUN_LIVE_LLM_TESTS=1 that verifies:
//   1. Tool-use round-trip: send a request with a tool definition, assert the
//      provider returns a ToolUse content block naming the correct tool.
//   2. 429 retry: use a test double that returns 429 twice then 200; assert the
//      provider retried and returned the final response.
//   3. Model normalisation: verify that model aliases (e.g. "claude" → canonical
//      ID) are resolved consistently.
//   4. Timeout propagation: inject a slow provider mock, assert the call fails
//      within the configured timeout_seconds window.
//
// Wire via a new `assert_inference_provider_live_contract(provider, rt)` function
// called from each provider's test module when the env var is set.
