generator looprs_desktop_rust {
  output_type rust
  output_dir "../src"
  version "0.218.0"
  default_client_mode async
  on_generate "cargo fmt --manifest-path ../../../Cargo.toml"
}
