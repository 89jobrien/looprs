# Testing Guide

## Running Tests

```bash
# All tests (fast)
cargo nextest run

# Specific crate
cargo nextest run -p looprs

# With coverage
cargo llvm-cov nextest --html

# Watch mode
cargo watch -x "nextest run"
```

## Test Categories

- **Unit**: `src/**/*.rs` - Fast, isolated component tests
- **Integration**: `tests/*.rs` - End-to-end workflows
- **Property**: Tests with `proptest!` macro - Randomized input testing
- **Snapshot**: Tests using `insta::assert_snapshot!` - Output regression testing
