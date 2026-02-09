# Advanced Error Patterns

## Error Downcasting

Convert from generic error to specific type:

```rust
use anyhow::Error;
use std::io;

fn handle_error(err: Error) {
    if let Some(io_err) = err.downcast_ref::<io::Error>() {
        match io_err.kind() {
            io::ErrorKind::NotFound => println!("File not found"),
            io::ErrorKind::PermissionDenied => println!("Permission denied"),
            _ => println!("Other IO error"),
        }
    }
}
```

## Multiple Error Types

Use enum for different error categories:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("validation error: {field}")]
    Validation { field: String },
}
```

## Error Recovery

Implement fallback strategies:

```rust
fn fetch_with_retry(url: &str, max_attempts: u32) -> Result<Response> {
    let mut attempts = 0;
    loop {
        match fetch(url) {
            Ok(resp) => return Ok(resp),
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

## Testing Error Conditions

Test error paths explicitly:

```rust
#[test]
fn test_invalid_input() {
    let result = parse_config("invalid");
    assert!(result.is_err());
    
    match result {
        Err(ConfigError::InvalidFormat(_)) => {},
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[test]
fn test_error_message() {
    let err = ConfigError::NotFound("config.toml".to_string());
    assert_eq!(err.to_string(), "config file not found: config.toml");
}
```

## Error Chains

Preserve full error context:

```rust
use anyhow::{Context, Result};

fn load_user_data(id: u64) -> Result<User> {
    let db = connect_db()
        .context("failed to connect to database")?;
    
    let user = db.query_user(id)
        .with_context(|| format!("failed to load user {}", id))?;
    
    Ok(user)
}
```

When this fails, you get the full chain:
```
Error: failed to load user 42
Caused by:
    0: database query failed
    1: connection timeout
```

## Custom Error Display

Implement custom formatting:

```rust
use std::fmt;

#[derive(Debug)]
pub struct ValidationError {
    field: String,
    message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Validation failed for '{}': {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}
```
