use looprs::ApiConfig;

#[test]
fn api_config_requires_env() {
    match ApiConfig::from_env() {
        Ok(_) => panic!("expected missing API key error"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(msg.contains("No API key"));
        }
    }
}
