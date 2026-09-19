use looprs::app_config::DefaultsConfig;
use looprs::{
    FsMode, ObservationQuery, ObservationStore, RuntimeSettings, SqliteObservationStore,
    StaticToolCatalog, ToolCatalog, ToolContext, ToolDefinition, ToolDispatcher, ToolError,
    ToolPorts, session_trace_path, trace_stream_is_stale,
};

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
