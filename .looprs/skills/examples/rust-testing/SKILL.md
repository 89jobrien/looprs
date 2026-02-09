---
name: rust-testing
description: Guide for writing effective Rust tests with cargo test, unit tests, integration tests, and test organization
triggers:
  - "rust test"
  - "cargo test"
  - "unit test"
  - "integration test"
  - "test coverage"
  - "#[test]"
---

# Rust Testing

Comprehensive guide for testing Rust code effectively.

## Quick Start

Write unit tests inline with your code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    #[should_panic(expected = "divide by zero")]
    fn test_panic() {
        divide(10, 0);
    }
}
```

Run tests with:
```bash
cargo test              # Run all tests
cargo test test_name    # Run specific test
cargo test -- --nocapture  # Show println output
```

## Test Organization

**Unit tests** - In the same file as the code:
- Use `#[cfg(test)]` module
- Test private functions
- Fast, isolated

**Integration tests** - In `tests/` directory:
- Test public API only
- Each file is a separate crate
- Example: `tests/integration_test.rs`

**Doc tests** - In documentation comments:
```rust
/// Adds two numbers
///
/// ```
/// assert_eq!(add(2, 2), 4);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

## Common Patterns

**Testing Results:**
```rust
#[test]
fn test_result() -> Result<(), String> {
    let value = parse_config("config.toml")?;
    assert_eq!(value.port, 8080);
    Ok(())
}
```

**Testing with setup/teardown:**
```rust
use tempfile::TempDir;

#[test]
fn test_with_cleanup() {
    let dir = TempDir::new().unwrap();
    // Test with temp directory
    // Automatically cleaned up on drop
}
```

**Async tests** (with tokio):
```rust
#[tokio::test]
async fn test_async() {
    let result = fetch_data().await;
    assert!(result.is_ok());
}
```

## Best Practices

1. **One assertion per test** - Makes failures clear
2. **Test edge cases** - Empty inputs, boundaries, errors
3. **Use descriptive names** - `test_parse_empty_string_returns_error`
4. **Avoid test interdependence** - Each test should be isolated
5. **Mock external dependencies** - Use traits for dependency injection

## Useful Attributes

- `#[test]` - Mark as test function
- `#[should_panic]` - Expect panic
- `#[ignore]` - Skip by default (run with `cargo test -- --ignored`)
- `#[cfg(test)]` - Compile only in test mode

## Coverage

Generate coverage with `cargo-tarpaulin`:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

Aim for >80% coverage on critical paths.
