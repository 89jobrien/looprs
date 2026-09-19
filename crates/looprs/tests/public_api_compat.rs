use looprs::app_config::DefaultsConfig;
use looprs::{
    FsMode, ObservationQuery, ObservationStore, RuntimeSettings, SqliteObservationStore,
    session_trace_path, trace_stream_is_stale,
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
