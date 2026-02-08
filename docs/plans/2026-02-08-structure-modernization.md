# Project Structure Modernization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure the crate for modern Rust best practices, improved testability, and clearer module boundaries without changing behavior.

**Architecture:** Introduce a library-first layout (`src/lib.rs`) with a thin CLI binary (`src/bin/looprs/main.rs`). Split tools into focused submodules, keep a minimal public API, and add targeted unit/integration tests.

**Tech Stack:** Rust 2024, `anyhow`, `thiserror`, `tokio`, `reqwest`, `serde`.

---

### Task 1: Add missing Cargo metadata and test dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add metadata fields**

```toml
[package]
name = "looprs"
version = "0.1.1"
edition = "2024"
description = "Concise coding assistant REPL"
license = "MIT"
repository = "https://example.com/looprs"
readme = "README.md"
rust-version = "1.78"
```

**Step 2: Add dev-dependency for tests**

```toml
[dev-dependencies]
tempfile = "3.12"
```

**Step 3: Verify formatting**

Run: `cargo metadata --no-deps`  
Expected: Success, shows new fields.

**Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add cargo metadata"
```

---

### Task 2: Move CLI parsing into bin crate with tests (TDD)

**Files:**
- Create: `src/bin/looprs/cli.rs`
- Modify: `src/bin/looprs/main.rs`
- Delete: `src/cli.rs`

**Step 1: Write failing unit tests (new module)**

Create `src/bin/looprs/cli.rs` with tests referencing `parse_input` (not yet implemented):

```rust
#[cfg(test)]
mod tests {
    use super::parse_input;
    use super::CliCommand;

    #[test]
    fn parse_quit_commands() {
        assert!(matches!(parse_input("/q"), Some(CliCommand::Quit)));
        assert!(matches!(parse_input("exit"), Some(CliCommand::Quit)));
        assert!(matches!(parse_input("quit"), Some(CliCommand::Quit)));
    }

    #[test]
    fn parse_clear_commands() {
        assert!(matches!(parse_input("/c"), Some(CliCommand::Clear)));
        assert!(matches!(parse_input("clear"), Some(CliCommand::Clear)));
    }

    #[test]
    fn parse_message_commands() {
        assert!(matches!(
            parse_input("hello"),
            Some(CliCommand::Message(_))
        ));
    }

    #[test]
    fn ignore_empty_input() {
        assert!(parse_input("").is_none());
        assert!(parse_input("   ").is_none());
    }
}
```

**Step 2: Run tests to see failure**

Run: `cargo test`  
Expected: FAIL (missing `parse_input`/`CliCommand`).

**Step 3: Implement CLI parsing in `src/bin/looprs/cli.rs`**

```rust
pub enum CliCommand {
    Quit,
    Clear,
    Message(String),
}

pub fn parse_input(line: &str) -> Option<CliCommand> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return None;
    }

    match trimmed {
        "/q" | "exit" | "quit" => Some(CliCommand::Quit),
        "/c" | "clear" => Some(CliCommand::Clear),
        msg => Some(CliCommand::Message(msg.to_string())),
    }
}
```

**Step 4: Wire CLI module into binary**

In `src/bin/looprs/main.rs`:

```rust
mod cli;
use cli::{parse_input, CliCommand};
```

**Step 5: Remove old CLI module**

Delete: `src/cli.rs`

**Step 6: Run tests**

Run: `cargo test`  
Expected: PASS (tests in `src/bin/looprs/cli.rs`).

**Step 7: Commit**

```bash
git add src/bin/looprs/cli.rs src/bin/looprs/main.rs src/cli.rs
git commit -m "refactor: move cli parsing into bin crate"
```

---

### Task 3: Introduce library crate and thin binary

**Files:**
- Create: `src/lib.rs`
- Create: `src/bin/looprs/main.rs`
- Delete: `src/main.rs`
- Modify: `src/agent.rs`, `src/api.rs`, `src/config.rs`, `src/errors.rs`, `src/tools/mod.rs`

**Step 1: Create `src/lib.rs` with minimal public API**

```rust
mod agent;
mod api;
mod config;
mod errors;
mod tools;

pub use crate::agent::Agent;
pub use crate::config::ApiConfig;
```

**Step 2: Create new binary entrypoint**

Create `src/bin/looprs/main.rs` by moving the contents of `src/main.rs`, then update imports to use the library:

```rust
use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;

use looprs::{Agent, ApiConfig};

mod cli;
use cli::{parse_input, CliCommand};

#[tokio::main]
async fn main() -> Result<()> {
    let config = ApiConfig::from_env()?;
    let mut agent = Agent::new(config.clone())?;
    let mut rl = DefaultEditor::new()?;

    println!(
        "{} {} | {} | {}",
        ">>".bold(),
        "looprs".bold(),
        config.model.cyan(),
        env::current_dir()?.display().to_string().dimmed()
    );
    println!("{}", "Commands: /q (quit), /c (clear history)".dimmed());

    loop {
        let readline = rl.readline(&format!("{} ", "❯".purple().bold()));

        match readline {
            Ok(line) => {
                let Some(command) = parse_input(&line) else {
                    continue;
                };

                let _ = rl.add_history_entry(&line);

                match command {
                    CliCommand::Quit => break,
                    CliCommand::Clear => {
                        agent.clear_history();
                        println!("{}", "● Conversation cleared".dimmed());
                    }
                    CliCommand::Message(msg) => {
                        agent.add_user_message(msg);

                        if let Err(e) = agent.run_turn().await {
                            eprintln!("\n{} {}", "✗".red().bold(), e.to_string().red());
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("\n{}", "Goodbye!".dimmed());
                break;
            }
            Err(e) => {
                eprintln!("Input error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}
```

**Step 3: Remove old `src/main.rs`**

Delete: `src/main.rs`

**Step 4: Run tests**

Run: `cargo test`  
Expected: PASS.

**Step 5: Commit**

```bash
git add src/lib.rs src/bin/looprs/main.rs src/main.rs
git commit -m "refactor: add lib crate and thin binary"
```

---

### Task 4: Split tools into submodules with tests (TDD)

**Files:**
- Create: `src/tools/mod.rs`
- Create: `src/tools/error.rs`
- Create: `src/tools/{read,write,edit,glob,grep,bash}.rs`
- Delete: `src/tools.rs`
- Modify: `src/agent.rs`

**Step 1: Create `src/tools/mod.rs` shell with module declarations**

```rust
pub mod error;
mod bash;
mod edit;
mod glob;
mod grep;
mod read;
mod write;

pub use error::ToolError;

use anyhow::Result;
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};

pub struct ToolContext {
    pub working_dir: PathBuf,
}

impl ToolContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            working_dir: env::current_dir()?,
        })
    }

    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }
}

pub fn execute_tool(name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    match name {
        "read" => read::tool_read(args, ctx),
        "write" => write::tool_write(args, ctx),
        "edit" => edit::tool_edit(args, ctx),
        "glob" => glob::tool_glob(args, ctx),
        "grep" => grep::tool_grep(args, ctx),
        "bash" => bash::tool_bash(args),
        _ => Err(ToolError::MissingParameter("Unknown tool")),
    }
}

pub fn get_tool_definitions() -> Vec<crate::api::ToolDefinition> {
    use serde_json::json;

    vec![
        crate::api::ToolDefinition {
            name: "read".into(),
            description: "Read file with line numbers. Supports offset and limit for pagination."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start from (0-indexed)",
                        "default": 0
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        },
        crate::api::ToolDefinition {
            name: "write".into(),
            description: "Write content to file (creates or overwrites). Parent directories are created if needed.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        crate::api::ToolDefinition {
            name: "edit".into(),
            description: "Replace text in file. The 'old' string must be unique unless all=true is set.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old": {
                        "type": "string",
                        "description": "Exact text to find and replace"
                    },
                    "new": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false)",
                        "default": false
                    }
                },
                "required": ["path", "old", "new"]
            }),
        },
        crate::api::ToolDefinition {
            name: "glob".into(),
            description: "Find files matching glob pattern. Results sorted by modification time (newest first).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pat": {
                        "type": "string",
                        "description": "Glob pattern (e.g., '*.rs', '**/*.toml')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory for search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["pat"]
            }),
        },
        crate::api::ToolDefinition {
            name: "grep".into(),
            description: "Search files for regex pattern. Returns up to 50 matches.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pat": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory for search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["pat"]
            }),
        },
        crate::api::ToolDefinition {
            name: "bash".into(),
            description: "Execute shell command. Returns stdout and stderr.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cmd": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["cmd"]
            }),
        },
    ]
}
```

**Step 2: Create `src/tools/error.rs` with `ToolError`**

Move from `src/errors.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Pattern '{0}' not found in file")]
    PatternNotFound(String),

    #[error("Pattern appears {0} times; use all=true or be more specific")]
    AmbiguousPattern(usize),

    #[error("Missing required parameter: {0}")]
    MissingParameter(&'static str),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),
}
```

**Step 3: Write failing tests for read/write/edit tools**

Create `src/tools/read.rs` with tests referencing `tool_read` (not yet implemented):

```rust
use crate::tools::error::ToolError;
use crate::tools::ToolContext;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_respects_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "a\nb\nc\n").unwrap();

        let ctx = ToolContext { working_dir: dir.path().to_path_buf() };
        let args = json!({"path": "a.txt", "offset": 1, "limit": 1});

        let out = tool_read(&args, &ctx).unwrap();
        assert!(out.contains("2| b"));
    }
}
```

**Step 4: Run tests to see failure**

Run: `cargo test`  
Expected: FAIL (missing `tool_read`).

**Step 5: Implement `tool_read` in `src/tools/read.rs`**

Move logic from `src/tools.rs` into this module, exporting `pub(super) fn tool_read(...)`.

**Step 6: Repeat for write/edit/glob/grep/bash**

For each module, create tests first, run tests (expect failure), then move implementation.

Suggested tests:
- `write`: writes content to a file and reads back.
- `edit`: replaces a unique string.
- `glob`: returns "none" for empty directory, and matches file names.
- `grep`: finds a known line.
- `bash`: runs `echo ok` and returns output; missing `cmd` returns error.

**Step 7: Update `src/agent.rs` imports**

Change imports to `use crate::tools::{execute_tool, get_tool_definitions, ToolContext};` (path remains, but module root changed).

**Step 8: Delete `src/tools.rs` and update `src/errors.rs`**

- Remove `src/tools.rs` file.
- Replace `src/errors.rs` with a module stub or delete if unused.

**Step 9: Run tests**

Run: `cargo test`  
Expected: PASS.

**Step 10: Commit**

```bash
git add src/tools src/agent.rs src/errors.rs
git commit -m "refactor: split tools into submodules"
```

---

### Task 5: Add CLI smoke integration test

**Files:**
- Create: `tests/cli_smoke.rs`

**Step 1: Write test**

```rust
use looprs::ApiConfig;

#[test]
fn api_config_requires_env() {
    let err = ApiConfig::from_env().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("No API key"));
}
```

**Step 2: Run tests**

Run: `cargo test`  
Expected: PASS.

**Step 3: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "test: add cli smoke test"
```

---

### Task 6: Final verification

**Step 1: Run full test suite**

Run: `cargo test`  
Expected: PASS.

**Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`  
Expected: PASS.

**Step 3: Summarize changes**

Note any warnings, failures, or changes needed.
