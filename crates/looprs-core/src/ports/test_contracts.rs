//! Conformance test suites for port traits.
//!
//! Each function asserts the semantic contract a trait promises. Call these
//! from any adapter's `#[cfg(test)]` module to prove the impl is correct.

use crate::observation::Observation;
use crate::ports::message_broker::{Message, MessageBroker};
use crate::ports::model_catalog::RemoteModelCatalogPort;
use crate::ports::observation_store::ObservationStore;
use crate::ports::plugin_runtime::{
    PluginHealthState, PluginKind, PluginSupervisorError, PluginSupervisorPort,
};
use crate::ports::session_store::{SessionEvent, SessionStore};
use crate::ports::user_output::UserOutput;

// Plugin supervisor lifecycle and error contracts.

/// Assert that a managed daemon satisfies the shared supervision contract.
///
/// The same contract applies to tool, runtime, and orchestration plugins:
/// status reports a live process, probe preserves health, restart replaces the
/// process and increments its bounded counter, and shutdown is observable.
pub fn assert_plugin_supervisor_contract(
    supervisor: &mut dyn PluginSupervisorPort,
    kind: PluginKind,
    plugin_name: &str,
) {
    let initial = supervisor
        .status(kind, plugin_name)
        .expect("managed daemon status must be available");
    assert_eq!(initial.kind, kind);
    assert_eq!(initial.plugin_name, plugin_name);
    assert_eq!(initial.state, PluginHealthState::Healthy);
    assert!(initial.pid.is_some(), "healthy daemon must expose its pid");

    let probed = supervisor
        .probe(kind, plugin_name)
        .expect("managed daemon probe must succeed");
    assert_eq!(probed.state, PluginHealthState::Healthy);

    supervisor
        .restart(kind, plugin_name, "conformance restart")
        .expect("managed daemon restart must succeed");
    let restarted = supervisor
        .status(kind, plugin_name)
        .expect("restarted daemon status must be available");
    assert_eq!(restarted.restart_count, initial.restart_count + 1);
    assert_eq!(
        restarted.last_restart_reason.as_deref(),
        Some("conformance restart")
    );
    assert_ne!(
        restarted.pid, initial.pid,
        "restart must replace the process"
    );

    supervisor
        .shutdown(kind, plugin_name)
        .expect("managed daemon shutdown must succeed");
    let stopped = supervisor
        .status(kind, plugin_name)
        .expect("stopped daemon status must remain observable");
    assert_eq!(stopped.state, PluginHealthState::Stopped);
    assert!(stopped.pid.is_none());
}

/// Assert shared unknown, one-shot, and disabled supervision errors.
pub fn assert_plugin_supervisor_error_contract(
    supervisor: &mut dyn PluginSupervisorPort,
    kind: PluginKind,
    one_shot_name: &str,
    disabled_name: &str,
) {
    let unknown = supervisor
        .restart(kind, "missing-conformance-plugin", "conformance")
        .expect_err("unknown plugin restart must fail");
    assert!(matches!(
        unknown,
        PluginSupervisorError::UnknownPlugin { .. }
    ));

    let one_shot = supervisor
        .restart(kind, one_shot_name, "conformance")
        .expect_err("one-shot plugin restart must fail");
    assert!(matches!(one_shot, PluginSupervisorError::NotDaemon { .. }));

    let disabled = supervisor
        .restart(kind, disabled_name, "conformance")
        .expect_err("disabled plugin restart must fail");
    assert!(matches!(disabled, PluginSupervisorError::Disabled { .. }));
    let disabled_status = supervisor
        .status(kind, disabled_name)
        .expect("disabled daemon status must remain observable");
    assert_eq!(disabled_status.state, PluginHealthState::Disabled);
    assert!(disabled_status.pid.is_none());
}

// Message delivery, fan-out, isolation, and closure contracts.

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

// Session identity, event logging, and path stability contracts.

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

// Inference provider metadata, error, and response contracts.

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
    if let Err(error) = provider.validate_config() {
        assert!(
            !error.to_string().trim().is_empty(),
            "validate_config() errors must carry a message"
        );
    }
}

/// Assert the shared semantics for a provider with invalid configuration.
///
/// Validation and inference must both return descriptive errors rather than
/// panicking or producing an apparently successful response.
pub async fn assert_inference_provider_error_contract(
    provider: &dyn crate::ports::InferenceProvider,
) {
    use crate::api::Message;

    let validation_error = provider
        .validate_config()
        .expect_err("invalid provider configuration must fail validation");
    assert!(
        !validation_error.to_string().trim().is_empty(),
        "validation errors must carry a message"
    );

    let request = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![Message::user("conformance error")],
        tools: Vec::new(),
        max_tokens: 16,
        temperature: None,
        system: String::new(),
    };
    let inference_error = provider
        .infer(&request)
        .await
        .expect_err("invalid provider configuration must not infer successfully");
    assert!(
        !inference_error.to_string().trim().is_empty(),
        "inference errors must carry a message"
    );
}

/// Validate the response semantics shared by all provider implementations.
///
/// Providers may emit assistant text and tool calls. Tool results are runtime
/// messages and must never appear in a provider response.
pub async fn assert_inference_provider_response_contract(
    provider: &dyn crate::ports::InferenceProvider,
) -> Result<(), String> {
    use crate::api::{ContentBlock, Message};

    let request = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![Message::user("conformance response")],
        tools: Vec::new(),
        max_tokens: 16,
        temperature: None,
        system: String::new(),
    };
    let response = provider
        .infer(&request)
        .await
        .map_err(|error| error.to_string())?;
    for block in &response.content {
        match block {
            ContentBlock::ToolUse { id, name, input }
                if id.as_str().trim().is_empty()
                    || name.as_str().trim().is_empty()
                    || !input.is_object() =>
            {
                return Err("provider response contained a malformed tool call".to_string());
            }
            ContentBlock::ToolResult { .. } => {
                return Err("provider response contained a tool result".to_string());
            }
            ContentBlock::Text { .. } | ContentBlock::ToolUse { .. } => {}
        }
    }
    Ok(())
}

// User output method safety contract.

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

// Observation persistence and repeated-save contracts.

/// Assert that an `ObservationStore` implementation satisfies the full contract.
///
/// Contract:
/// 1. Saving an empty batch succeeds.
/// 2. Saving a batch of observations (all fields populated, including
///    optional ones) succeeds.
/// 3. Repeated saves of the same batch succeed — persistence must be
///    idempotent-tolerant, not fail on duplicates.
pub fn assert_observation_store_contract(store: &dyn ObservationStore) {
    // 1. Empty batch
    store
        .save(&[])
        .expect("save() must accept an empty observation batch");

    // 2. Fully populated batch
    let obs_with_id = Observation::new(
        "bash".into(),
        serde_json::json!({"command": "echo hi"}),
        "hi".to_string(),
        Some(crate::types::ToolId::new("tu-1")),
        "sess-conformance".to_string(),
    )
    .with_context("conformance capture".to_string());
    let obs_minimal = Observation::new(
        "read".into(),
        serde_json::json!({}),
        String::new(),
        None,
        "sess-conformance".to_string(),
    );
    let batch = vec![obs_with_id, obs_minimal];
    store
        .save(&batch)
        .expect("save() must accept a fully populated batch");

    // 3. Repeated save of the same data must not error
    store
        .save(&batch)
        .expect("repeated save() of identical observations must not error");
}

// Remote model catalog source and result-validation contracts.

/// Assert that a `RemoteModelCatalogPort` implementation satisfies the full contract.
///
/// Contract:
/// 1. `source()` is callable and stable across calls.
/// 2. `list_models()` for any provider returns either models with non-empty
///    provider/model strings, or a structured `RemoteCatalogError`.
/// 3. An empty provider name does not panic.
pub async fn assert_remote_model_catalog_contract(catalog: &dyn RemoteModelCatalogPort) {
    // 1. Stable source
    let _ = catalog.source();

    for provider in ["anthropic", "openai", ""] {
        match catalog.list_models(provider).await {
            Ok(models) => {
                for model in &models {
                    assert!(
                        !model.provider.is_empty(),
                        "list_models({provider:?}) returned a model with empty provider"
                    );
                    assert!(
                        !model.model.is_empty(),
                        "list_models({provider:?}) returned a model with empty model id"
                    );
                }
            }
            Err(err) => {
                assert!(
                    !err.message.is_empty(),
                    "list_models({provider:?}) error must carry a message"
                );
            }
        }
    }
}

/// Run the legacy low-cost live provider smoke contract.
///
/// This helper is not gated: calling it always performs one provider request.
/// The caller owns opt-in policy, credential setup, and test isolation. Keep
/// the calling test ignored by default and check `LOOPRS_RUN_LIVE_LLM_TESTS`
/// before invoking this helper.
///
/// ```no_run
/// use looprs_core::ports::{InferenceProvider, test_contracts};
///
/// async fn run_live_contract(provider: &dyn InferenceProvider) {
///     if std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS").as_deref() != Ok("1") {
///         return;
///     }
///
///     test_contracts::assert_inference_provider_live_contract(provider).await;
/// }
/// ```
pub async fn assert_inference_provider_live_contract(
    provider: &dyn crate::ports::InferenceProvider,
) {
    use crate::api::Message;

    let single_turn = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![Message::user("Reply with the single word: pong")],
        tools: vec![],
        max_tokens: 64,
        temperature: Some(0.0),
        system: String::new(),
    };
    let response = provider
        .infer(&single_turn)
        .await
        .expect("live contract single-turn inference must succeed");
    assert_valid_inference_response(&response, "single-turn");
}

/// Run the opt-in live provider scenario matrix.
///
/// The matrix performs two requests for providers without tool support and
/// four requests for providers with tool support. Each invocation can incur
/// provider charges and can fail because of credentials, quotas, networking,
/// model availability, or nondeterministic model behavior. Keep it ignored by
/// default. This helper does not inspect `LOOPRS_RUN_LIVE_LLM_TESTS`; the caller
/// must apply the same opt-in gate shown on
/// [`assert_inference_provider_live_contract`] before invoking it.
pub async fn assert_inference_provider_live_matrix(provider: &dyn crate::ports::InferenceProvider) {
    use crate::api::{ContentBlock, Message, ToolDefinition};

    assert_inference_provider_live_contract(provider).await;

    let multi_turn = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![
            Message::user("What is the capital of France?"),
            Message::assistant(vec![ContentBlock::Text {
                text: "Paris".to_string(),
            }]),
            Message::user("Which country is that city in?"),
        ],
        tools: vec![],
        max_tokens: 64,
        temperature: Some(0.0),
        system: String::new(),
    };
    let response = provider
        .infer(&multi_turn)
        .await
        .expect("live contract multi-turn inference must succeed");
    assert_valid_inference_response(&response, "multi-turn");

    if !provider.supports_tool_use() {
        return;
    }

    let tool_request = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![Message::user(
            "Use get_weather to get the weather in Paris. Do not answer without calling it.",
        )],
        tools: vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the current weather for a city".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }),
        }],
        max_tokens: 128,
        temperature: Some(0.0),
        system: "Always use an available tool when asked.".to_string(),
    };
    let tool_response = provider
        .infer(&tool_request)
        .await
        .expect("live contract tool-use inference must succeed");
    assert_valid_usage(&tool_response, "tool-use");

    let tool_ids = tool_response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !tool_ids.is_empty(),
        "live contract tool-use response must contain a tool call"
    );

    let tool_results = tool_ids
        .into_iter()
        .map(|tool_use_id| ContentBlock::ToolResult {
            tool_use_id,
            content: "sunny".to_string(),
        })
        .collect();
    let follow_up = crate::ports::InferenceRequest {
        model: provider.model().clone(),
        messages: vec![
            tool_request.messages[0].clone(),
            Message::assistant(tool_response.content),
            Message::tool_results(tool_results),
        ],
        tools: tool_request.tools,
        max_tokens: 128,
        temperature: Some(0.0),
        system: tool_request.system,
    };
    let response = provider
        .infer(&follow_up)
        .await
        .expect("live contract tool-result follow-up must succeed");
    assert_valid_inference_response(&response, "tool-result follow-up");
}

fn assert_valid_inference_response(response: &crate::ports::InferenceResponse, scenario: &str) {
    use crate::api::ContentBlock;

    assert!(
        response
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Text { text } if !text.is_empty())),
        "live contract {scenario} response must contain non-empty assistant text"
    );
    assert_valid_usage(response, scenario);
}

fn assert_valid_usage(response: &crate::ports::InferenceResponse, scenario: &str) {
    assert!(
        response.usage.input_tokens > 0,
        "live contract {scenario} usage.input_tokens must be > 0"
    );
    assert!(
        response.usage.output_tokens > 0,
        "live contract {scenario} usage.output_tokens must be > 0"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ContentBlock;
    use crate::ports::model_catalog::{CatalogSource, RemoteCatalogError, RemoteModel};
    use crate::ports::{InferenceProvider, InferenceRequest, InferenceResponse, Usage};
    use crate::types::{ModelId, ToolId, ToolName};
    use futures::StreamExt;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct ScriptedInferenceProvider {
        model: ModelId,
        requests: Mutex<Vec<InferenceRequest>>,
        responses: Mutex<VecDeque<InferenceResponse>>,
        supports_tools: bool,
    }

    struct FailingInferenceProvider {
        model: ModelId,
    }

    struct InvalidInferenceProvider {
        model: ModelId,
        response: InferenceResponse,
    }

    #[async_trait::async_trait]
    impl InferenceProvider for FailingInferenceProvider {
        async fn infer(
            &self,
            _req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            Err("scripted inference failure".into())
        }

        fn name(&self) -> &str {
            "failing-scripted"
        }

        fn model(&self) -> &ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("scripted validation failure".into())
        }
    }

    #[async_trait::async_trait]
    impl InferenceProvider for InvalidInferenceProvider {
        async fn infer(
            &self,
            _req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            Ok(self.response.clone())
        }

        fn name(&self) -> &str {
            "invalid-scripted"
        }

        fn model(&self) -> &ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("scripted validation failure".into())
        }
    }

    impl ScriptedInferenceProvider {
        fn new(supports_tools: bool) -> Self {
            let mut responses =
                VecDeque::from([text_response("pong"), text_response("Paris is in France")]);
            if supports_tools {
                responses.push_back(InferenceResponse {
                    content: vec![ContentBlock::ToolUse {
                        id: ToolId::new("call-1"),
                        name: ToolName::new("get_weather"),
                        input: serde_json::json!({"city": "Paris"}),
                    }],
                    stop_reason: "tool_use".to_string(),
                    usage: Usage {
                        input_tokens: 3,
                        output_tokens: 2,
                    },
                });
                responses.push_back(text_response("sunny"));
            }
            Self {
                model: ModelId::new("contract-model"),
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses),
                supports_tools,
            }
        }
    }

    fn text_response(text: &str) -> InferenceResponse {
        InferenceResponse {
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 3,
                output_tokens: 2,
            },
        }
    }

    #[async_trait::async_trait]
    impl InferenceProvider for ScriptedInferenceProvider {
        async fn infer(
            &self,
            req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            self.requests.lock().unwrap().push(req.clone());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| "contract made an unexpected inference call".into())
        }

        fn name(&self) -> &str {
            "scripted"
        }

        fn model(&self) -> &ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn supports_tool_use(&self) -> bool {
            self.supports_tools
        }
    }

    /// Reference in-memory ObservationStore used to validate the contract
    /// suite itself. Also serves as a reusable test double for other tests.
    pub struct MemObservationStore {
        saved: std::sync::Mutex<Vec<Vec<Observation>>>,
    }

    impl MemObservationStore {
        /// Creates an empty in-memory observation store.
        pub fn new() -> Self {
            Self {
                saved: std::sync::Mutex::new(Vec::new()),
            }
        }

        /// Returns the number of observation batches saved by the store.
        pub fn batch_count(&self) -> usize {
            self.saved.lock().unwrap().len()
        }
    }

    impl Default for MemObservationStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ObservationStore for MemObservationStore {
        fn save(&self, observations: &[Observation]) -> Result<(), anyhow::Error> {
            self.saved.lock().unwrap().push(observations.to_vec());
            Ok(())
        }
    }

    /// Reference RemoteModelCatalogPort fake backed by a static table.
    pub struct FakeCatalog;

    #[async_trait::async_trait]
    impl RemoteModelCatalogPort for FakeCatalog {
        async fn list_models(
            &self,
            provider: &str,
        ) -> Result<Vec<RemoteModel>, RemoteCatalogError> {
            if provider.is_empty() {
                return Err(RemoteCatalogError {
                    provider: provider.to_string(),
                    message: "provider name must not be empty".to_string(),
                });
            }
            Ok(vec![RemoteModel {
                provider: provider.to_string(),
                model: format!("{provider}-fake-model"),
                source: CatalogSource::GistFallback,
            }])
        }

        fn source(&self) -> CatalogSource {
            CatalogSource::LiveApi
        }
    }

    #[test]
    fn observation_store_contract_holds_for_reference_fake() {
        let store = MemObservationStore::new();
        assert_observation_store_contract(&store);
        assert_eq!(store.batch_count(), 3, "contract performs exactly 3 saves");
    }

    #[tokio::test]
    async fn remote_model_catalog_contract_holds_for_reference_fake() {
        assert_remote_model_catalog_contract(&FakeCatalog).await;
    }

    #[tokio::test]
    async fn inference_live_contract_exercises_shared_scenario_matrix() {
        let provider = ScriptedInferenceProvider::new(true);

        assert_inference_provider_live_matrix(&provider).await;

        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 4, "all inference scenarios must run");
        assert_eq!(requests[0].messages.len(), 1, "single-turn scenario");
        assert_eq!(requests[1].messages.len(), 3, "multi-turn scenario");
        assert_eq!(requests[2].tools.len(), 1, "tool-use scenario");
        assert!(
            requests[3]
                .messages
                .iter()
                .flat_map(|message| &message.content)
                .any(|block| matches!(block, ContentBlock::ToolResult { .. })),
            "tool result must round-trip into the follow-up request"
        );
    }

    #[tokio::test]
    async fn inference_live_contract_skips_tools_when_unsupported() {
        let provider = ScriptedInferenceProvider::new(false);

        assert_inference_provider_live_matrix(&provider).await;

        assert_eq!(provider.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn legacy_live_contract_remains_a_single_low_cost_request() {
        let provider = ScriptedInferenceProvider::new(false);

        assert_inference_provider_live_contract(&provider).await;

        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    #[should_panic(expected = "single-turn response must contain non-empty assistant text")]
    async fn inference_live_matrix_rejects_empty_text_deterministically() {
        let provider = ScriptedInferenceProvider {
            model: ModelId::new("contract-model"),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::from([InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: String::new(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            }])),
            supports_tools: false,
        };

        assert_inference_provider_live_matrix(&provider).await;
    }

    #[tokio::test]
    #[should_panic(expected = "single-turn usage.input_tokens must be > 0")]
    async fn inference_live_matrix_rejects_zero_usage_deterministically() {
        let provider = ScriptedInferenceProvider {
            model: ModelId::new("contract-model"),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::from([InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: "pong".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 0,
                    output_tokens: 1,
                },
            }])),
            supports_tools: false,
        };

        assert_inference_provider_live_matrix(&provider).await;
    }

    #[tokio::test]
    async fn default_stream_emits_typed_delta_and_authoritative_final_response() {
        let provider = ScriptedInferenceProvider::new(false);
        let request = InferenceRequest {
            model: provider.model().clone(),
            messages: vec![crate::api::Message::user("stream")],
            tools: Vec::new(),
            max_tokens: 32,
            temperature: Some(0.0),
            system: "stream".to_string(),
        };

        let events = provider
            .infer_stream(&request)
            .await
            .collect::<Vec<_>>()
            .await;

        assert_eq!(provider.requests.lock().unwrap().len(), 1);
        assert!(matches!(
            events.first(),
            Some(Ok(crate::ports::InferenceStreamEvent::Delta(
                crate::ports::InferenceDelta::Text(text)
            ))) if text == "pong"
        ));
        assert!(matches!(
            events.last(),
            Some(Ok(crate::ports::InferenceStreamEvent::Final(response)))
                if response.usage.input_tokens == 3 && response.usage.output_tokens == 2
        ));
    }

    #[tokio::test]
    async fn default_stream_preserves_inference_errors_without_a_final_response() {
        let provider = FailingInferenceProvider {
            model: ModelId::new("failing-model"),
        };
        let request = InferenceRequest {
            model: provider.model().clone(),
            messages: vec![crate::api::Message::user("stream")],
            tools: Vec::new(),
            max_tokens: 32,
            temperature: None,
            system: String::new(),
        };

        let events = provider
            .infer_stream(&request)
            .await
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].as_ref().unwrap_err().to_string(),
            "scripted inference failure"
        );
    }

    #[tokio::test]
    async fn provider_error_contract_covers_validation_and_inference_failures() {
        let provider = FailingInferenceProvider {
            model: ModelId::new("failing-model"),
        };

        assert_inference_provider_error_contract(&provider).await;
    }

    #[tokio::test]
    async fn provider_response_contract_rejects_tool_results_from_providers() {
        let provider = InvalidInferenceProvider {
            model: ModelId::new("invalid-model"),
            response: InferenceResponse {
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: ToolId::new("call-1"),
                    content: "provider-generated result".to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage::default(),
            },
        };

        let error = assert_inference_provider_response_contract(&provider)
            .await
            .unwrap_err();

        assert!(error.contains("tool result"));
    }

    #[tokio::test]
    async fn provider_response_contract_rejects_malformed_tool_calls() {
        let provider = InvalidInferenceProvider {
            model: ModelId::new("invalid-model"),
            response: InferenceResponse {
                content: vec![ContentBlock::ToolUse {
                    id: ToolId::new(""),
                    name: ToolName::new("read"),
                    input: serde_json::json!(["README.md"]),
                }],
                stop_reason: "tool_use".to_string(),
                usage: Usage::default(),
            },
        };

        let error = assert_inference_provider_response_contract(&provider)
            .await
            .unwrap_err();

        assert!(error.contains("malformed tool call"));
    }
}
