# Testing Guide for looprs

## Quick Start

```bash
# Run all tests (recommended)
just test-all

# Run specific test suite
cargo nextest run -p looprs-desktop --test ui::root_tests

# Run with coverage
just test-coverage
```

## Test Organization

```
looprs/
├── tests/                          # Integration tests
│   ├── cli_smoke.rs
│   └── model_precedence.rs
├── crates/looprs-desktop/
│   ├── src/
│   │   └── **/*.rs                 # Unit tests in #[cfg(test)] modules
│   └── tests/
│       ├── ui/                     # Freya UI tests (headless)
│       │   ├── root_tests.rs
│       │   ├── editor_tests.rs
│       │   └── terminal_tests.rs
│       ├── sqlite_store_tests.rs  # Database tests
│       ├── mockstation_tests.rs   # Service tests
│       ├── property_tests.rs      # Property-based tests
│       └── integration/
│           └── desktop_flow.rs    # End-to-end tests
└── benches/                        # Performance benchmarks
    └── desktop_benchmarks.rs
```

## Test Categories

### Unit Tests
Fast, isolated tests for individual functions/methods.

```rust
#[test]
fn test_parse_rgb_valid() {
    assert_eq!(parse_rgb("rgb(255, 0, 0)"), Some((255, 0, 0)));
}
```

### Integration Tests
Test interactions between multiple components.

```rust
#[tokio::test]
async fn test_chat_persistence_workflow() {
    append_chat_message("User", "Hello").await;
    let messages = load_chat_messages(10).await;
    assert_eq!(messages.len(), 1);
}
```

### UI Tests (Headless)
Test Freya UI components without rendering to screen.

```rust
#[test]
fn test_button_click() {
    let mut test = TestingRunner::new(my_component());
    test.sync_and_update();
    test.click_cursor((100.0, 50.0));
    assert_text_contains(&test, "Clicked");
}
```

### Property-Based Tests
Test invariants with randomized inputs using proptest.

```rust
proptest! {
    #[test]
    fn test_truncate_never_grows(
        input in ".*",
        max_chars in 0usize..1000
    ) {
        let (result, _) = truncate_string(&input, max_chars);
        assert!(result.len() <= input.len());
    }
}
```

### Snapshot Tests
Verify outputs don't change unexpectedly using insta.

```rust
#[test]
fn test_component_render() {
    let code = render_component_code(&tree);
    insta::assert_snapshot!(code);
}
```

## Running Tests

### All Tests
```bash
# Using nextest (fast, parallel)
just test-all
cargo nextest run

# Using standard cargo test
cargo test
```

### Specific Test Suites
```bash
# Desktop UI tests only
just test-ui

# Database tests only
just test-db

# Service layer tests
just test-services

# Property-based tests
just test-property

# Integration tests
just test-integration
```

### With Coverage
```bash
# Generate HTML coverage report
just test-coverage

# View coverage report
open target/llvm-cov/html/index.html
```

### Watch Mode
```bash
# Auto-run tests on file changes
just test-watch
```

### Benchmarks
```bash
# Run all benchmarks
just bench

# Run specific benchmark
cargo bench json_merge
```

## Test Configuration

### Nextest Configuration
Located in `.config/nextest.toml`:

```toml
[profile.default]
retries = 2              # Retry flaky tests
fail-fast = false        # Run all tests even if some fail
test-threads = "num-cpus"  # Use all CPU cores

[profile.ci]
retries = 3              # More retries in CI
fail-fast = true         # Stop early on failure in CI
```

### Live LLM Tests
Tests that call external LLM APIs are ignored by default. To run them:

```bash
export LOOPRS_RUN_LIVE_LLM_TESTS=1
export OPENAI_API_KEY=your-key-here
cargo test --ignored
```

## Writing Good Tests

### DO ✅
- Test behavior, not implementation
- Use descriptive test names: `test_chat_persists_after_clear`
- Clean up resources (use TempDir for file tests)
- Use property tests for edge cases
- Keep tests independent (no shared state)

### DON'T ❌
- Test private implementation details
- Use `unwrap()` without context (use `expect("clear error message")`)
- Share mutable state between tests
- Ignore intermittent failures ("flaky" tests)
- Skip cleanup (always use RAII guards or defer cleanup)

## Common Testing Patterns

### Testing Async Code
```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_function().await;
    assert_eq!(result, expected);
}
```

### Testing with Temporary Files
```rust
#[test]
fn test_file_operation() {
    let tmp = TempDir::new().unwrap();
    std::env::set_var("DATA_DIR", tmp.path());

    // Test code here

    // Cleanup happens automatically when tmp is dropped
}
```

### Testing UI Components
```rust
#[test]
fn test_ui_component() {
    let mut test = TestingRunner::new(app());
    test.sync_and_update();

    // Click at coordinates
    test.click_cursor((100.0, 50.0));
    test.sync_and_update();

    // Verify text appears
    assert_text_contains(&test, "Expected text");
}
```

### Snapshot Testing
```rust
#[test]
fn test_output_snapshot() {
    let output = generate_output();
    insta::assert_snapshot!(output);
}

// To update snapshots after intentional changes:
// cargo insta test --accept
```

## CI Integration

Tests run automatically on:
- Every pull request
- Every push to main
- Coverage reports upload to Codecov

### CI Configuration
Located in `.github/workflows/ci.yml`:

- **Test Suite**: All tests with nextest + doctests
- **Coverage**: Generates lcov.info and uploads to Codecov
- **Clippy**: Lints with `-D warnings` (fails on warnings)
- **Rust Cache**: Uses Swatinem/rust-cache for faster builds

## Test Coverage Goals

**Current Coverage** (as of implementation):
- Core library: 80%+ (already high from existing tests)
- Desktop UI: 60%+ target (from 0%)
- Services layer: 70%+ target
- SQLite store: 90%+ target (from 0%)
- **Overall**: 65%+ target

**Critical Paths** (aim for 100%):
- Database operations (SQLite store)
- State management (UI root component)
- Message routing (Mockstation)

## Troubleshooting

### Tests Hang
Check for deadlocks in async code. Use `tokio::time::timeout`:

```rust
#[tokio::test]
async fn test_with_timeout() {
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        some_async_function()
    ).await;

    assert!(result.is_ok(), "Test timed out");
}
```

### Flaky Tests
Enable retries in `.config/nextest.toml` or use `tokio-test` helpers:

```rust
use tokio_test::assert_ok;

#[tokio::test]
async fn test_eventually_succeeds() {
    assert_ok!(retry_with_backoff(|| async_operation()).await);
}
```

### Coverage Gaps
```bash
# Generate coverage report
just test-coverage

# Open in browser
open target/llvm-cov/html/index.html

# Look for red/yellow highlighted lines
```

### Snapshot Mismatches
```bash
# Review snapshot diffs
cargo insta test

# Accept all changes (after reviewing)
just test-update-snapshots

# Review individual snapshots
cargo insta review
```

## Installing Test Tools

```bash
# Install all testing tools at once
just install-test-tools

# Or install individually:
cargo install cargo-nextest --locked
cargo install cargo-llvm-cov --locked
cargo install cargo-watch --locked
cargo install cargo-insta --locked
```

## Performance Benchmarking

### Running Benchmarks
```bash
# All benchmarks
just bench

# Specific benchmark group
cargo bench json_merge

# Save baseline for comparison
cargo bench --bench desktop_benchmarks -- --save-baseline main

# Compare against baseline
cargo bench --bench desktop_benchmarks -- --baseline main
```

### Benchmark Organization
Benchmarks are located in `crates/looprs-desktop/benches/`:

- `json_merge`: JSON merging performance
- `truncate_string`: String truncation with Unicode
- `parse_functions`: RGB and size parsing
- `escape_rsx_string`: String escaping

## Resources

- [cargo-nextest docs](https://nexte.st/)
- [Proptest book](https://proptest-rs.github.io/proptest/)
- [Insta snapshot testing](https://insta.rs/)
- [Freya testing guide](https://docs.freyaui.dev/freya_testing/)
- [SQLx testing documentation](https://docs.rs/sqlx/latest/sqlx/attr.test.html)
- [Tokio testing utilities](https://docs.rs/tokio-test/)
