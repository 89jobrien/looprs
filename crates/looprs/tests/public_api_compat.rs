//! Compile-checks compatibility aliases and the supported public embedding API.

use looprs::adapters::{PluginsAdapter, RetryProvider, TerminalOutput};
use looprs::app_config::DefaultsConfig;
use looprs::file_refs::FileRefPolicy;
use looprs::plugins::{MockRunner, Plugins, ToolResolver};
use looprs::ports::UserOutput;
use looprs::providers::{InferenceRequest, InferenceResponse, LLMProvider, Usage};
use looprs::{
    FsMode, ModelId, ObservationQuery, ObservationStore, PluginExecutor, RuntimeSettings,
    SqliteObservationStore, StaticToolCatalog, ToolCatalog, ToolContext, ToolDefinition,
    ToolDispatcher, ToolError, ToolPorts, has_file_references, list_file_references,
    resolve_file_references, session_trace_path, trace_stream_is_stale,
};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn runtime_settings_support_forward_compatible_construction() {
    let settings = RuntimeSettings::new(DefaultsConfig::default(), Some(1_024), FsMode::Read)
        .with_max_parallel(4)
        .with_mcp_server_url("http://127.0.0.1:3000");

    assert_eq!(settings.max_parallel(), 4);
    assert_eq!(settings.mcp_server_url(), Some("http://127.0.0.1:3000"));
}

#[test]
fn observation_and_trace_review_apis_are_public() {
    fn assert_observation_ports<T: ObservationStore + ObservationQuery>() {}
    assert_observation_ports::<SqliteObservationStore>();

    let base = std::path::Path::new("traces");
    assert_eq!(
        session_trace_path(base, "session"),
        base.join("session.jsonl")
    );
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    assert!(trace_stream_is_stale(&missing, std::time::UNIX_EPOCH).unwrap());
}

#[test]
fn tool_port_composition_is_public_to_embedding_consumers() {
    struct Dispatcher;

    #[async_trait::async_trait]
    impl ToolDispatcher for Dispatcher {
        async fn execute(
            &self,
            _name: &str,
            _args: &serde_json::Value,
            _ctx: &ToolContext,
        ) -> Result<String, ToolError> {
            Ok("ok".to_string())
        }
    }

    let catalog: std::sync::Arc<dyn ToolCatalog> =
        std::sync::Arc::new(StaticToolCatalog::new(vec![ToolDefinition {
            name: "embedded".to_string(),
            description: "embedding-defined tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }]));
    let dispatcher: std::sync::Arc<dyn ToolDispatcher> = std::sync::Arc::new(Dispatcher);

    let _ports = ToolPorts::new(catalog, dispatcher);
}

struct EmbeddedProvider {
    model: ModelId,
}

#[async_trait::async_trait]
impl LLMProvider for EmbeddedProvider {
    async fn infer(
        &self,
        _request: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
        Ok(InferenceResponse {
            content: Vec::new(),
            stop_reason: "complete".to_string(),
            usage: Usage::default(),
        })
    }

    fn name(&self) -> &str {
        "embedded"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[tokio::test]
async fn retained_retry_provider_is_usable_without_network_access() {
    let provider = RetryProvider::new(EmbeddedProvider {
        model: ModelId::new("embedded-model"),
    })
    .with_base_delay_ms(0);
    let request = InferenceRequest {
        model: ModelId::new("embedded-model"),
        messages: Vec::new(),
        tools: Vec::new(),
        max_tokens: 1,
        temperature: None,
        system: String::new(),
    };

    let response = provider.infer(&request).await.unwrap();

    assert_eq!(provider.name(), "embedded");
    assert_eq!(provider.model().as_str(), "embedded-model");
    assert_eq!(response.stop_reason, "complete");
}

struct StaticResolver;

impl ToolResolver for StaticResolver {
    fn resolve(&self, tool: &str) -> Option<PathBuf> {
        (tool == "embedded-tool").then(|| PathBuf::from("/virtual/embedded-tool"))
    }
}

#[test]
fn retained_plugin_test_apis_compose_without_spawning_processes() {
    let runner = Arc::new(MockRunner::new());
    let plugins = Plugins::new(runner.clone(), Arc::new(StaticResolver));
    let adapter = PluginsAdapter::new(&plugins);

    assert!(adapter.has_tool("embedded-tool"));
    let error = adapter
        .execute_tool("embedded-tool", vec!["--version".into()])
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    let calls = runner.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].program, PathBuf::from("/virtual/embedded-tool"));
}

#[test]
fn retained_terminal_output_and_file_reference_apis_are_public() {
    fn assert_user_output<T: UserOutput>() {}
    assert_user_output::<TerminalOutput>();

    assert!(has_file_references("review @notes.txt"));
    assert_eq!(list_file_references("@a.rs and @b.md"), ["a.rs", "b.md"]);

    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("notes.txt"), "embedded content").unwrap();
    let resolved = resolve_file_references(
        "review @notes.txt",
        directory.path(),
        &FileRefPolicy::default(),
    )
    .unwrap();
    assert!(resolved.contains("embedded content"));
}
