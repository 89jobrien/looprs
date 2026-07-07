# Testing Guide

## Running Tests

```bash
# Workspace tests
cargo nextest run --workspace

# Specific crate
cargo nextest run -p looprs

# CLI binary tests
cargo nextest run -p looprs-cli --bin looprs

# Local pre-push gate
cargo xtask pre-push

# With coverage
cargo llvm-cov nextest --workspace --html

# Watch mode
cargo watch -x "nextest run"
```

`cargo xtask pre-push` delegates to `taskit pre-push` and then runs the `looprs-cli` binary tests separately. This matters because the `looprs-cli` package has a lightweight library target for workspace tooling, while its argument parsing and CLI behavior tests live under the `looprs` binary target.

## Test Categories

- **Unit**: crate-local `#[cfg(test)]` modules under `crates/looprs-core/`, `crates/looprs/`, `crates/looprs-cli/`, and `xtask/`.
- **Property**: randomized boundary tests using `proptest` in parser, sanitizer, badge, and adapter areas.
- **Fuzz**: libFuzzer targets under `fuzz/fuzz_targets/`; these are excluded from the default workspace and should be run explicitly when changing parser or text-processing boundaries.
- **Model check**: no active model-checking target is currently wired into local gates.
- **Conformance**: reusable port contract helpers live in `crates/looprs-core/src/ports/test_contracts.rs`.
- **Integration**: root-level workflows in `tests/*.rs`, plus CLI binary tests in `crates/looprs-cli/src/args.rs` and `crates/looprs-cli/src/cli.rs`.
- **Regression**: focused tests for previously fragile behavior, including model precedence, session logging, protocol drift surfaces, and CLI argument handling.

## Local Gate Expectations

- Use `cargo nextest run --workspace` for the main workspace suite.
- Use `cargo nextest run -p looprs-cli --bin looprs` when validating CLI parsing or startup behavior directly.
- Use `cargo clippy --all-targets --all-features -- -D warnings` for warning-free Rust changes.
- Use `cargo xtask pre-push` before pushing; it combines `taskit` propagation/protocol checks with the CLI binary tests.
