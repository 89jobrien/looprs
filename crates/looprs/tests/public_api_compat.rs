use looprs::app_config::DefaultsConfig;
use looprs::{FsMode, RuntimeSettings};

#[test]
fn runtime_settings_support_forward_compatible_construction() {
    let settings = RuntimeSettings::new(DefaultsConfig::default(), Some(1_024), FsMode::Read)
        .with_max_parallel(4)
        .with_mcp_server_url("http://127.0.0.1:3000");

    assert_eq!(settings.max_parallel(), 4);
    assert_eq!(settings.mcp_server_url(), Some("http://127.0.0.1:3000"));
}
