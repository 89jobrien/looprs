---
name: rust-error-handling
description: Guide for Rust error handling with Result<T,E>, error propagation, the ? operator, and custom error types using thiserror and anyhow. Use when working with Rust code that needs error handling, Result types, error conversion, or custom error implementations.
---

# Rust Error Handling

Guide for effective error handling in Rust.

## Quick Start

Use `Result<T, E>` for operations that can fail:

```rust
use std::fs;
use std::io;

fn read_username(path: &str) -> Result<String, io::Error> {
    let content = fs::read_to_string(path)?;
    Ok(content.trim().to_string())
}
```

The `?` operator:
- Returns early on `Err`
- Automatically converts errors via `From` trait
- Only works in functions returning `Result` or `Option`

## Error Propagation

Chain operations with `?`:

```rust
use anyhow::Result;

fn load_config() -> Result<Config> {
    let path = find_config_file()?;
    let content = fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}
```

## Custom Error Types

**For libraries - use `thiserror`:**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),
    
    #[error("invalid config format")]
    InvalidFormat(#[from] toml::de::Error),
    
    #[error("io error")]
    Io(#[from] std::io::Error),
}
```

**For applications - use `anyhow`:**

```rust
use anyhow::{Context, Result};

fn process() -> Result<()> {
    let config = load_config()
        .context("failed to load config")?;
    
    let data = fetch_data(&config.url)
        .context("failed to fetch data")?;
    
    Ok(())
}
```

## Error Context

Add context to errors for better debugging:

```rust
use anyhow::{Context, Result};

fn parse_port(s: &str) -> Result<u16> {
    s.parse()
        .with_context(|| format!("failed to parse port: {}", s))
}
```

## Advanced Patterns

See `references/error_patterns.md` for:
- Error downcasting
- Multiple error types
- Error recovery strategies
- Testing error conditions

Use `scripts/check_error_handling.sh` to verify error handling coverage in your codebase.

## Best Practices

1. **Use `Result` for recoverable errors, `panic!` for bugs**
2. **Add context with `.context()`** - Makes debugging easier
3. **Define custom errors for libraries** - Better API ergonomics
4. **Use `anyhow` for applications** - Simpler error handling
5. **Test error paths** - Don't just test the happy path
