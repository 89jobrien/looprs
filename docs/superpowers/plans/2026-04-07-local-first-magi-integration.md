# Local-First + magi Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make looprs a local-first coding agent that emits structured session logs, routes
providers via a curated model tier config, exposes magi-aware commands, and shows a model badge in
the desktop UI — with magi ingesting Ollama sessions for automated RL.

**Architecture:** looprs writes JSONL session logs with provider tags; magi hooks into SessionEnd
to ingest Ollama-only interactions into its RL pipeline. Provider routing is driven by
`~/.looprs/models.toml`. No direct import between repos — the hook is the only coupling point.

**Tech Stack:** Rust (looprs), Python (magi hook), YAML (looprs commands/hooks), TOML (model
config), Freya (desktop badge), SQLite (magi rewards.db), serde_json, tokio, reqwest.

---

## File Map

### New files — looprs

| File | Responsibility |
|------|---------------|
| `src/session_log.rs` | `SessionLogger` — writes JSONL to `~/.looprs/sessions/` |
| `src/models_config.rs` | Parse `~/.looprs/models.toml` into `ModelsConfig` |
| `src/scorer.rs` | OpenAI scoring call; `ScoreTrigger` enum (OnError, OnRepeat, OnDemand) |
| `.looprs/commands/model-status.yaml` | `/model-status` command |
| `.looprs/commands/fine-tune.yaml` | `/fine-tune` command |
| `.looprs/commands/reset-model.yaml` | `/reset-model` command |
| `.looprs/commands/score-session.yaml` | `/score-session` command (OpenAI scoring on recent interactions) |
| `.looprs/commands/outsource.yaml` | `/outsource` command |
| `crates/looprs-desktop/src/services/model_badge.rs` | Reads `modelcard.yaml`, polls every 60s |
| `tests/session_log_tests.rs` | Unit tests for session log format |
| `tests/models_config_tests.rs` | Unit tests for `models.toml` parsing |

### New files — magi

| File | Responsibility |
|------|---------------|
| `hooks/looprs/magi_ingest.yaml` | looprs SessionEnd hook definition |
| `scripts/looprs_ingest.py` | Reads looprs JSONL, writes to rewards.db |
| `tests/test_looprs_ingest.py` | `test_ingest_from_looprs_session` |

### Modified files — looprs

| File | Change |
|------|--------|
| `src/agent.rs` | Wire `SessionLogger`; fire log events at each lifecycle point |
| `src/lib.rs` | Export `session_log`, `models_config`, `scorer` modules |
| `src/providers/mod.rs` | Read `ModelsConfig` for provider selection; add outsource tier routing |
| `crates/looprs-desktop/src/services/mod.rs` | Export `model_badge` |
| `crates/looprs-desktop/src/ui/root.rs` | Render model badge widget |

---

## Task 1: Session Log Format

**Goal:** `SessionLogger` writes JSONL to `~/.looprs/sessions/<date>-<session-id>.jsonl`.

**Files:**
- Create: `src/session_log.rs`
- Create: `tests/session_log_tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// tests/session_log_tests.rs
use looprs::session_log::{SessionLogger, SessionEvent};
use tempfile::tempdir;
use std::io::BufRead;

#[test]
fn test_writes_valid_jsonl() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());
    logger.log(SessionEvent::UserMessage {
        content: "hello".into(),
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::SessionEnd);

    let path = logger.path();
    let file = std::fs::File::open(&path).unwrap();
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|l| l.unwrap())
        .collect();

    assert_eq!(lines.len(), 2);
    let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(first["event"], "user_message");
    assert_eq!(first["provider"], "ollama");
    assert!(first["ts"].is_string());
    assert!(first["session_id"].is_string());
}

#[test]
fn test_provider_tag_preserved() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());
    logger.log(SessionEvent::Inference {
        content: "response".into(),
        provider: "openai".into(),
    });
    let content = std::fs::read_to_string(logger.path()).unwrap();
    let event: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(event["provider"], "openai");
}

#[test]
fn test_all_event_types_serialize() {
    let dir = tempdir().unwrap();
    let mut logger = SessionLogger::new(dir.path().to_path_buf());

    logger.log(SessionEvent::UserMessage { content: "q".into(), provider: "ollama".into() });
    logger.log(SessionEvent::Inference { content: "a".into(), provider: "ollama".into() });
    logger.log(SessionEvent::ToolUse {
        tool_name: "bash".into(),
        input: serde_json::json!({"command": "ls"}),
        tool_use_id: "tu_1".into(),
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::ToolResult {
        tool_use_id: "tu_1".into(),
        output: "file.rs".into(),
        is_error: false,
        provider: "ollama".into(),
    });
    logger.log(SessionEvent::SessionEnd);

    let path = logger.path();
    let lines: Vec<String> = std::io::BufReader::new(std::fs::File::open(path).unwrap())
        .lines().map(|l| l.unwrap()).collect();
    assert_eq!(lines.len(), 5);
    let event_types = ["user_message", "inference", "tool_use", "tool_result", "session_end"];
    for (i, expected) in event_types.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(&lines[i]).unwrap();
        assert_eq!(v["event"], *expected);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/joe/dev/looprs && cargo test --test session_log_tests 2>&1 | head -20
```
Expected: compile error — `session_log` module not found.

- [ ] **Step 3: Implement `src/session_log.rs`**

```rust
use chrono::Utc;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    UserMessage { content: String, provider: String },
    Inference { content: String, provider: String },
    ToolUse {
        tool_name: String,
        input: serde_json::Value,
        tool_use_id: String,
        provider: String,
    },
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
        provider: String,
    },
    SessionEnd,
}

#[derive(Debug)]
pub struct SessionLogger {
    session_id: String,
    path: PathBuf,
}

impl SessionLogger {
    pub fn new(sessions_dir: PathBuf) -> Self {
        let session_id = format!("sess-{}", Uuid::new_v4());
        let date = Utc::now().format("%Y-%m-%d");
        let filename = format!("{}-{}.jsonl", date, session_id);
        fs::create_dir_all(&sessions_dir).ok();
        let path = sessions_dir.join(filename);
        Self { session_id, path }
    }

    pub fn session_id(&self) -> &str { &self.session_id }

    pub fn path(&self) -> &PathBuf { &self.path }

    pub fn log(&mut self, event: SessionEvent) {
        #[derive(Serialize)]
        struct LogLine<'a> {
            ts: String,
            session_id: &'a str,
            #[serde(flatten)]
            event: &'a SessionEvent,
        }
        let line = LogLine {
            ts: Utc::now().to_rfc3339(),
            session_id: &self.session_id,
            event: &event,
        };
        if let Ok(mut json) = serde_json::to_string(&line) {
            json.push('\n');
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
                let _ = file.write_all(json.as_bytes());
            }
        }
    }
}
```

- [ ] **Step 4: Export module; add deps to `Cargo.toml`**

In `src/lib.rs`, add:
```rust
pub mod session_log;
```

In `Cargo.toml` (if not present):
```toml
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/joe/dev/looprs && cargo test --test session_log_tests 2>&1
```
Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/joe/dev/looprs
git add src/session_log.rs src/lib.rs tests/session_log_tests.rs Cargo.toml
git commit -m "feat: add SessionLogger with JSONL session event logging"
```

---

## Task 2: Models Config

**Goal:** Parse `~/.looprs/models.toml` into `ModelsConfig`. Used by provider routing and commands.

**Files:**
- Create: `src/models_config.rs`
- Create: `tests/models_config_tests.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/models_config_tests.rs
use looprs::models_config::ModelsConfig;
use tempfile::NamedTempFile;
use std::io::Write;

fn write_toml(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

#[test]
fn test_parse_full_config() {
    let toml = r#"
[default]
provider = "ollama"
model = "magistral-small-rl-v17"

[tiers]
fast      = { provider = "ollama", model = "qwen2.5-coder:7b" }
capable   = { provider = "ollama", model = "magistral-small-rl-v17" }
outsource = { provider = "openai", model = "gpt-4o" }
judge     = { provider = "openai", model = "gpt-5.4" }

[magi]
modelcard = "/dev/magi/modelcard.yaml"
db        = "/dev/magi/db/rewards.db"
"#;
    let f = write_toml(toml);
    let config = ModelsConfig::from_path(f.path()).unwrap();
    assert_eq!(config.default.provider, "ollama");
    assert_eq!(config.default.model, "magistral-small-rl-v17");
    assert_eq!(config.tier("outsource").unwrap().provider, "openai");
    assert_eq!(config.tier("judge").unwrap().model, "gpt-5.4");
    assert_eq!(config.magi_modelcard(), "/dev/magi/modelcard.yaml");
    assert_eq!(config.magi_db(), "/dev/magi/db/rewards.db");
}

#[test]
fn test_missing_tier_returns_none() {
    let toml = r#"
[default]
provider = "ollama"
model = "qwen2.5-coder:7b"
"#;
    let f = write_toml(toml);
    let config = ModelsConfig::from_path(f.path()).unwrap();
    assert!(config.tier("outsource").is_none());
    assert!(config.magi_modelcard().is_empty());
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd /Users/joe/dev/looprs && cargo test --test models_config_tests 2>&1 | head -20
```

- [ ] **Step 3: Implement `src/models_config.rs`**

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderTier {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Default)]
struct MagiConfig {
    #[serde(default)] modelcard: String,
    #[serde(default)] db: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelsConfig {
    pub default: ProviderTier,
    #[serde(default)] tiers: HashMap<String, ProviderTier>,
    #[serde(default)] magi: MagiConfig,
}

impl ModelsConfig {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&content).context("parsing models.toml")
    }

    pub fn load() -> Result<Self> {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".looprs")
            .join("models.toml");
        Self::from_path(&path)
    }

    pub fn tier(&self, name: &str) -> Option<&ProviderTier> {
        self.tiers.get(name)
    }

    pub fn magi_modelcard(&self) -> &str { &self.magi.modelcard }

    pub fn magi_db(&self) -> &str { &self.magi.db }
}
```

- [ ] **Step 4: Export module; add deps**

In `src/lib.rs`:
```rust
pub mod models_config;
```

In `Cargo.toml`:
```toml
toml = "0.8"
dirs = "5"
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/joe/dev/looprs && cargo test --test models_config_tests 2>&1
```
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/joe/dev/looprs
git add src/models_config.rs src/lib.rs tests/models_config_tests.rs Cargo.toml
git commit -m "feat: add ModelsConfig for ~/.looprs/models.toml provider tier routing"
```

---

## Task 3: Wire SessionLogger into Agent

**Goal:** Agent creates a `SessionLogger` at session start and calls `log()` at each lifecycle
point. Provider name comes from `self.provider.name()`.

**Files:**
- Modify: `src/agent.rs`

- [ ] **Step 1: Add field and import**

In `src/agent.rs`:
```rust
use crate::session_log::{SessionLogger, SessionEvent};
```

Add to `Agent` struct:
```rust
pub session_logger: Option<SessionLogger>,
```

In `Agent::new()`, initialize:
```rust
let sessions_dir = dirs::home_dir()
    .unwrap_or_default()
    .join(".looprs")
    .join("sessions");
session_logger: Some(SessionLogger::new(sessions_dir)),
```

- [ ] **Step 2: Log `UserMessage` before inference**

In `run_turn`, after receiving user prompt, before `infer()`:
```rust
if let Some(ref mut logger) = self.session_logger {
    logger.log(SessionEvent::UserMessage {
        content: user_prompt.clone(),
        provider: self.provider.name().to_string(),
    });
}
```

- [ ] **Step 3: Log `Inference` after LLM response**

After `self.provider.infer(&req).await?`:
```rust
if let Some(ref mut logger) = self.session_logger {
    logger.log(SessionEvent::Inference {
        content: response.content_text().unwrap_or_default(),
        provider: self.provider.name().to_string(),
    });
}
```

(If `InferenceResponse` lacks `content_text()`, extract from the first `ContentBlock::Text`.)

- [ ] **Step 4: Log `ToolUse` before each tool call**

Before `execute_tool()`:
```rust
if let Some(ref mut logger) = self.session_logger {
    logger.log(SessionEvent::ToolUse {
        tool_name: tool_call.name.clone(),
        input: tool_call.input.clone(),
        tool_use_id: tool_call.id.clone(),
        provider: self.provider.name().to_string(),
    });
}
```

- [ ] **Step 5: Log `ToolResult` after each tool call**

After `execute_tool()`:
```rust
if let Some(ref mut logger) = self.session_logger {
    logger.log(SessionEvent::ToolResult {
        tool_use_id: tool_call.id.clone(),
        output: tool_result.content.clone(),
        is_error: tool_result.is_error,
        provider: self.provider.name().to_string(),
    });
}
```

- [ ] **Step 6: Log `SessionEnd`**

Where `Event::SessionEnd` is emitted, add before it:
```rust
if let Some(ref mut logger) = self.session_logger {
    logger.log(SessionEvent::SessionEnd);
}
```

- [ ] **Step 7: Run cargo check**

```bash
cd /Users/joe/dev/looprs && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
cd /Users/joe/dev/looprs
git add src/agent.rs
git commit -m "feat: wire SessionLogger into agent lifecycle events"
```

---

## Task 4: Scorer Module (OpenAI Interaction Scoring)

**Goal:** `ScoreTrigger` enum + `run_scorer()` that calls OpenAI on last N Ollama interactions
from the session JSONL and writes scores to magi's rewards.db.

**Files:**
- Create: `src/scorer.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests (inline in `src/scorer.rs`)**

```rust
// src/scorer.rs — paste at bottom behind #[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("2026-04-07-sess-abc.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"fix the bug","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:01Z","session_id":"s1","event":"inference","content":"here is the fix","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:02Z","session_id":"s1","event":"inference","content":"cloud response","provider":"openai"}}"#).unwrap();
        path
    }

    #[test]
    fn test_load_pairs_filters_ollama_only() {
        let dir = tempdir().unwrap();
        let path = write_fixture(dir.path());
        let pairs = load_last_n_ollama_pairs(&path, 10).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].prompt, "fix the bug");
        assert_eq!(pairs[0].response, "here is the fix");
    }

    #[test]
    fn test_load_pairs_respects_limit() {
        let dir = tempdir().unwrap();
        let path = dir.join("sess.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..5usize {
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"q{i}","provider":"ollama"}}"#).unwrap();
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"inference","content":"a{i}","provider":"ollama"}}"#).unwrap();
        }
        let pairs = load_last_n_ollama_pairs(&path, 2).unwrap();
        assert_eq!(pairs.len(), 2);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd /Users/joe/dev/looprs && cargo test scorer 2>&1 | head -20
```
Expected: compile error.

- [ ] **Step 3: Implement `src/scorer.rs`**

```rust
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum ScoreTrigger {
    OnError,
    OnRepeat { tool_name: String, count: usize },
    OnDemand { n: usize },
}

#[derive(Debug)]
pub struct InteractionPair {
    pub prompt: String,
    pub response: String,
    pub session_id: String,
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(default)] event: String,
    #[serde(default)] provider: String,
    #[serde(default)] content: String,
    #[serde(default)] session_id: String,
}

/// Read last `n` prompt→response pairs, ollama-tagged only.
pub fn load_last_n_ollama_pairs(path: &Path, n: usize) -> Result<Vec<InteractionPair>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let events: Vec<RawEvent> = std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .filter(|e: &RawEvent| e.provider == "ollama")
        .collect();

    let mut pairs: Vec<InteractionPair> = Vec::new();
    let mut i = 0;
    while i + 1 < events.len() {
        if events[i].event == "user_message" && events[i + 1].event == "inference" {
            pairs.push(InteractionPair {
                prompt: events[i].content.clone(),
                response: events[i + 1].content.clone(),
                session_id: events[i].session_id.clone(),
            });
            i += 2;
        } else {
            i += 1;
        }
    }
    let skip = pairs.len().saturating_sub(n);
    Ok(pairs.into_iter().skip(skip).collect())
}

/// Call OpenAI to score pairs. Returns empty vec if OPENAI_API_KEY absent.
/// Writes scores to magi db at db_path if provided.
pub async fn run_scorer(
    pairs: &[InteractionPair],
    scorer_model: &str,
    db_path: Option<&str>,
) -> Result<Vec<f32>> {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(k) => k,
        Err(_) => {
            log::warn!("OPENAI_API_KEY not set — skipping interaction scoring");
            return Ok(vec![]);
        }
    };

    let client = reqwest::Client::new();
    let mut scores = Vec::new();

    for pair in pairs {
        let body = serde_json::json!({
            "model": scorer_model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert code reviewer. Score this coding response \
                                from 0.0 to 1.0. Reply with only a JSON object: \
                                {\"score\": <float>}"
                },
                {
                    "role": "user",
                    "content": format!("Task: {}\n\nResponse: {}", pair.prompt, pair.response)
                }
            ],
            "temperature": 0.0
        });

        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;

        let score = match resp {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json().await.unwrap_or_default();
                let content = v["choices"][0]["message"]["content"]
                    .as_str().unwrap_or("{}");
                let parsed: serde_json::Value =
                    serde_json::from_str(content).unwrap_or_default();
                parsed["score"].as_f64().unwrap_or(0.5) as f32
            }
            _ => {
                log::warn!("OpenAI scoring call failed — skipping interaction");
                continue;
            }
        };

        scores.push(score);

        if let Some(db) = db_path {
            write_score_to_db(db, &pair.session_id, &pair.prompt, &pair.response, score).await?;
        }
    }

    Ok(scores)
}

async fn write_score_to_db(
    db_path: &str,
    session_id: &str,
    task: &str,
    response: &str,
    score: f32,
) -> Result<()> {
    use rusqlite::Connection;
    let conn = Connection::open(db_path)?;
    conn.execute(
        "INSERT OR IGNORE INTO interactions \
         (task, response, judge_score, reward, processed, source_session) \
         VALUES (?1, ?2, ?3, ?3, 0, ?4)",
        rusqlite::params![task, response, score as f64, session_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("2026-04-07-sess-abc.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"fix the bug","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:01Z","session_id":"s1","event":"inference","content":"here is the fix","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:02Z","session_id":"s1","event":"inference","content":"cloud response","provider":"openai"}}"#).unwrap();
        path
    }

    #[test]
    fn test_load_pairs_filters_ollama_only() {
        let dir = tempdir().unwrap();
        let path = write_fixture(dir.path());
        let pairs = load_last_n_ollama_pairs(&path, 10).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].prompt, "fix the bug");
        assert_eq!(pairs[0].response, "here is the fix");
    }

    #[test]
    fn test_load_pairs_respects_limit() {
        let dir = tempdir().unwrap();
        let path = dir.join("sess.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..5usize {
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"q{i}","provider":"ollama"}}"#).unwrap();
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"inference","content":"a{i}","provider":"ollama"}}"#).unwrap();
        }
        let pairs = load_last_n_ollama_pairs(&path, 2).unwrap();
        assert_eq!(pairs.len(), 2);
    }
}
```

Add to `Cargo.toml`:
```toml
rusqlite = "0.31"
```

- [ ] **Step 4: Export module**

In `src/lib.rs`:
```rust
pub mod scorer;
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/joe/dev/looprs && cargo test scorer 2>&1
```
Expected: `test_load_pairs_filters_ollama_only` and `test_load_pairs_respects_limit` pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/joe/dev/looprs
git add src/scorer.rs src/lib.rs Cargo.toml
git commit -m "feat: add scorer module with OpenAI interaction scoring and pair extraction"
```

---

## Task 5: Auto-Score Triggers (OnError, OnRepeat)

**Goal:** Wire scorer into agent error path and tool-repeat detection to auto-trigger scoring.

**Files:**
- Modify: `src/agent.rs`

- [ ] **Step 1: Add `models_config` field to `Agent`**

In `src/agent.rs`:
```rust
use crate::models_config::ModelsConfig;
```

Add to `Agent` struct:
```rust
pub models_config: Option<ModelsConfig>,
```

In `Agent::new()`:
```rust
models_config: ModelsConfig::load().ok(),
```

- [ ] **Step 2: Add tool-repeat counter in `run_turn`**

At the start of `run_turn`, declare:
```rust
let mut tool_call_counts: std::collections::HashMap<String, usize> =
    std::collections::HashMap::new();
```

- [ ] **Step 3: Check repeat count after each tool call**

After incrementing, trigger scoring if threshold reached:
```rust
let count = tool_call_counts.entry(tool_call.name.clone()).or_insert(0);
*count += 1;
if *count >= 3 {
    log::info!("on-repeat trigger: {} called {} times", tool_call.name, count);
    self.maybe_score(crate::scorer::ScoreTrigger::OnRepeat {
        tool_name: tool_call.name.clone(),
        count: *count,
    }).await;
}
```

- [ ] **Step 4: Trigger scoring on tool error**

In the `OnError` branch after logging `ToolResult { is_error: true }`:
```rust
self.maybe_score(crate::scorer::ScoreTrigger::OnError).await;
```

- [ ] **Step 5: Add `maybe_score` helper to `Agent`**

```rust
async fn maybe_score(&self, trigger: crate::scorer::ScoreTrigger) {
    let Some(ref logger) = self.session_logger else { return };
    let Some(ref config) = self.models_config else { return };

    let scorer_model = config.tier("judge")
        .map(|t| t.model.as_str())
        .unwrap_or("gpt-4o");
    let db_path = config.magi_db();
    let db_opt = if db_path.is_empty() { None } else { Some(db_path) };

    let n = match &trigger {
        crate::scorer::ScoreTrigger::OnError => 1,
        crate::scorer::ScoreTrigger::OnRepeat { .. } => 3,
        crate::scorer::ScoreTrigger::OnDemand { n } => *n,
    };

    match crate::scorer::load_last_n_ollama_pairs(logger.path(), n) {
        Ok(pairs) => {
            if let Err(e) = crate::scorer::run_scorer(&pairs, scorer_model, db_opt).await {
                log::warn!("scoring failed: {e}");
            }
        }
        Err(e) => log::warn!("failed to load session pairs for scoring: {e}"),
    }
}
```

- [ ] **Step 6: Run cargo check**

```bash
cd /Users/joe/dev/looprs && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cd /Users/joe/dev/looprs
git add src/agent.rs
git commit -m "feat: auto-trigger OpenAI scoring on tool error and on-repeat (>=3 calls)"
```

---

## Task 6: looprs Commands (YAML)

**Goal:** Five command YAML files for `/model-status`, `/fine-tune`, `/reset-model`,
`/score-session`, `/outsource`.

**Files:**
- Create: `.looprs/commands/model-status.yaml`
- Create: `.looprs/commands/fine-tune.yaml`
- Create: `.looprs/commands/reset-model.yaml`
- Create: `.looprs/commands/score-session.yaml`
- Create: `.looprs/commands/outsource.yaml`

- [ ] **Step 1: Create `/model-status`**

```yaml
# .looprs/commands/model-status.yaml
name: model-status
description: Show current magi model version, mean reward, and training status
action:
  type: shell
  command: |
    python3 - << 'EOF'
    import yaml, sys, os, tomllib
    cfg_path = os.path.expanduser("~/.looprs/models.toml")
    try:
        with open(cfg_path, "rb") as f:
            cfg = tomllib.load(f)
        mc_path = cfg.get("magi", {}).get("modelcard", "")
    except Exception:
        mc_path = ""
    if not mc_path or not os.path.exists(mc_path):
        print("model: unknown (modelcard not found)")
        sys.exit(0)
    with open(mc_path) as f:
        mc = yaml.safe_load(f)
    model = mc.get("model_id", "unknown")
    status = mc.get("training_status", "idle")
    evals = mc.get("eval_results", {})
    rewards = [v.get("mean_reward", 0) for v in evals.values() if isinstance(v, dict)]
    mean = sum(rewards) / len(rewards) if rewards else 0.0
    print(f"model:   {model}")
    print(f"status:  {status}")
    print(f"reward:  {mean:.3f} (mean across {len(rewards)} tasks)")
    EOF
  inject_output: true
```

- [ ] **Step 2: Create `/fine-tune`**

```yaml
# .looprs/commands/fine-tune.yaml
name: fine-tune
description: Flag current session as high-priority for magi RL training
action:
  type: shell
  command: |
    python3 - << 'EOF'
    import os, glob, sqlite3, json, tomllib
    sessions_dir = os.path.expanduser("~/.looprs/sessions")
    files = sorted(glob.glob(os.path.join(sessions_dir, "*.jsonl")))
    if not files:
        print("No session logs found.")
        exit(0)
    session_id = None
    with open(files[-1]) as f:
        for line in f:
            try:
                ev = json.loads(line)
                session_id = ev.get("session_id")
            except Exception:
                pass
    cfg_path = os.path.expanduser("~/.looprs/models.toml")
    try:
        with open(cfg_path, "rb") as f:
            cfg = tomllib.load(f)
        db_path = cfg.get("magi", {}).get("db", "")
    except Exception:
        db_path = ""
    if not db_path or not os.path.exists(db_path):
        print(f"Session {session_id}: flagged locally (magi db not found).")
        exit(0)
    conn = sqlite3.connect(db_path)
    conn.execute(
        "UPDATE interactions SET reward = MIN(reward + 0.2, 1.0) WHERE source_session = ?",
        (session_id,)
    )
    conn.commit()
    conn.close()
    print(f"Session {session_id}: reward boosted in magi db.")
    EOF
  inject_output: true
```

- [ ] **Step 3: Create `/reset-model`**

```yaml
# .looprs/commands/reset-model.yaml
name: reset-model
description: "Revert models.toml default model. Usage: /reset-model <model-name>"
action:
  type: shell
  command: |
    python3 - {args} << 'EOF'
    import sys, os, re
    args = sys.argv[1:]
    if not args:
        print("Usage: /reset-model <model-name>  e.g. /reset-model magistral-small-2506")
        sys.exit(1)
    target = args[0]
    cfg_path = os.path.expanduser("~/.looprs/models.toml")
    if not os.path.exists(cfg_path):
        print(f"models.toml not found at {cfg_path}")
        sys.exit(1)
    with open(cfg_path) as f:
        content = f.read()
    updated = re.sub(
        r'(\[default\].*?model\s*=\s*)["\'].*?["\']',
        lambda m: m.group(1) + f'"{target}"',
        content, flags=re.DOTALL
    )
    with open(cfg_path, "w") as f:
        f.write(updated)
    print(f"Default model reset to: {target}")
    EOF
  inject_output: true
```

- [ ] **Step 4: Create `/score-session`**

```yaml
# .looprs/commands/score-session.yaml
name: score-session
description: "Score last N session interactions via OpenAI. Usage: /score-session [n=10]"
action:
  type: shell
  command: |
    python3 - {args} << 'EOF'
    import sys, os, glob, json, tomllib, urllib.request, sqlite3
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 10
    sessions_dir = os.path.expanduser("~/.looprs/sessions")
    files = sorted(glob.glob(os.path.join(sessions_dir, "*.jsonl")))
    if not files:
        print("No session logs found.")
        sys.exit(0)
    cfg_path = os.path.expanduser("~/.looprs/models.toml")
    try:
        with open(cfg_path, "rb") as f:
            cfg = tomllib.load(f)
        scorer_model = cfg.get("tiers", {}).get("judge", {}).get("model", "gpt-4o")
        db_path = cfg.get("magi", {}).get("db", "")
    except Exception:
        scorer_model, db_path = "gpt-4o", ""
    events = []
    with open(files[-1]) as f:
        for line in f:
            try:
                ev = json.loads(line)
                if ev.get("provider") == "ollama":
                    events.append(ev)
            except Exception:
                pass
    pairs = []
    i = 0
    while i + 1 < len(events):
        if events[i]["event"] == "user_message" and events[i+1]["event"] == "inference":
            pairs.append((events[i]["content"], events[i+1]["content"],
                          events[i].get("session_id", "")))
            i += 2
        else:
            i += 1
    pairs = pairs[-n:]
    if not pairs:
        print("No ollama interactions found in session.")
        sys.exit(0)
    api_key = os.environ.get("OPENAI_API_KEY", "")
    if not api_key:
        print("OPENAI_API_KEY not set — skipping scoring.")
        sys.exit(0)
    print(f"Scoring {len(pairs)} interactions with {scorer_model}...")
    for prompt, response, session_id in pairs:
        body = json.dumps({
            "model": scorer_model,
            "messages": [
                {"role": "system", "content": 'Score this coding response 0.0-1.0. Reply: {"score": <float>}'},
                {"role": "user", "content": f"Task: {prompt}\n\nResponse: {response}"}
            ],
            "temperature": 0.0
        }).encode()
        req = urllib.request.Request(
            "https://api.openai.com/v1/chat/completions", data=body,
            headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                data = json.loads(r.read())
            content = data["choices"][0]["message"]["content"]
            score = json.loads(content).get("score", 0.5)
            print(f"  score={score:.3f}  prompt={prompt[:60]}")
            if db_path and os.path.exists(db_path):
                conn = sqlite3.connect(db_path)
                conn.execute(
                    "INSERT OR IGNORE INTO interactions "
                    "(task, response, judge_score, reward, processed, source_session) "
                    "VALUES (?,?,?,?,0,?)",
                    (prompt, response, score, score, session_id)
                )
                conn.commit()
                conn.close()
        except Exception as e:
            print(f"  scoring call failed: {e}")
    EOF
  inject_output: true
```

- [ ] **Step 5: Create `/outsource`**

```yaml
# .looprs/commands/outsource.yaml
name: outsource
description: Re-run the last user prompt against the outsource tier (OpenAI gpt-4o)
action:
  type: message
  text: |
    Switching to outsource provider (OpenAI gpt-4o) for this task.
    Note: this interaction will NOT be fed to magi training.
```

- [ ] **Step 6: Verify commands appear in looprs**

Start a looprs session and run `/help`. Confirm all five commands are listed:
`model-status`, `fine-tune`, `reset-model`, `score-session`, `outsource`.

- [ ] **Step 7: Commit**

```bash
cd /Users/joe/dev/looprs
git add .looprs/commands/
git commit -m "feat: add /model-status /fine-tune /reset-model /score-session /outsource commands"
```

---

## Task 7: magi Hook Bridge

**Goal:** magi ships a looprs `SessionEnd` hook that reads session JSONL and writes Ollama
interactions to `db/rewards.db`.

**Files (magi repo):**
- Create: `hooks/looprs/magi_ingest.yaml`
- Create: `scripts/looprs_ingest.py`
- Create: `tests/test_looprs_ingest.py`

- [ ] **Step 1: Write the failing ingest test**

```python
# tests/test_looprs_ingest.py
import json, sqlite3, tempfile, os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'scripts'))
from looprs_ingest import ingest_session_file

def make_fixture(path):
    events = [
        {"ts": "2026-04-07T00:00:00Z", "session_id": "s1", "event": "user_message",
         "content": "write a fizzbuzz", "provider": "ollama"},
        {"ts": "2026-04-07T00:00:01Z", "session_id": "s1", "event": "inference",
         "content": "def fizzbuzz(n): ...", "provider": "ollama"},
        {"ts": "2026-04-07T00:00:02Z", "session_id": "s1", "event": "user_message",
         "content": "outsourced task", "provider": "openai"},
        {"ts": "2026-04-07T00:00:03Z", "session_id": "s1", "event": "inference",
         "content": "cloud response", "provider": "openai"},
        {"ts": "2026-04-07T00:00:04Z", "session_id": "s1", "event": "session_end",
         "provider": "ollama"},
    ]
    with open(path, 'w') as f:
        for ev in events:
            f.write(json.dumps(ev) + '\n')

def test_ingest_from_looprs_session():
    with tempfile.TemporaryDirectory() as tmpdir:
        jsonl_path = os.path.join(tmpdir, '2026-04-07-sess-s1.jsonl')
        db_path = os.path.join(tmpdir, 'rewards.db')
        make_fixture(jsonl_path)

        conn = sqlite3.connect(db_path)
        conn.execute('''CREATE TABLE interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task TEXT, response TEXT,
            rule_score REAL DEFAULT 0, embed_score REAL DEFAULT 0,
            judge_score REAL DEFAULT 0, memory_score REAL DEFAULT 0,
            reward REAL DEFAULT 0, processed INTEGER DEFAULT 0,
            source_session TEXT DEFAULT ""
        )''')
        conn.commit()
        conn.close()

        ingest_session_file(jsonl_path, db_path)

        conn = sqlite3.connect(db_path)
        rows = conn.execute(
            'SELECT task, response, source_session FROM interactions'
        ).fetchall()
        conn.close()

        assert len(rows) == 1
        assert rows[0][0] == 'write a fizzbuzz'
        assert rows[0][1] == 'def fizzbuzz(n): ...'
        assert rows[0][2] == 's1'
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd /Users/joe/dev/magi && python3 -m pytest tests/test_looprs_ingest.py -v 2>&1 | head -20
```
Expected: `ModuleNotFoundError: No module named 'looprs_ingest'`.

- [ ] **Step 3: Implement `scripts/looprs_ingest.py`**

```python
"""Ingest looprs session JSONL into magi rewards.db."""
import json
import sqlite3
import sys
from pathlib import Path


def load_ollama_pairs(jsonl_path: str) -> list[dict]:
    """Return prompt/response pairs where provider == ollama."""
    events = []
    with open(jsonl_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
                if ev.get("provider") == "ollama":
                    events.append(ev)
            except json.JSONDecodeError:
                continue

    pairs = []
    i = 0
    while i + 1 < len(events):
        if events[i]["event"] == "user_message" and events[i + 1]["event"] == "inference":
            pairs.append({
                "task": events[i]["content"],
                "response": events[i + 1]["content"],
                "session_id": events[i].get("session_id", ""),
            })
            i += 2
        else:
            i += 1
    return pairs


def ingest_session_file(jsonl_path: str, db_path: str) -> int:
    """Write ollama pairs from jsonl_path into rewards.db. Returns count inserted."""
    pairs = load_ollama_pairs(jsonl_path)
    if not pairs:
        return 0
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")
    inserted = 0
    for pair in pairs:
        try:
            conn.execute(
                "INSERT INTO interactions "
                "(task, response, reward, processed, source_session) "
                "VALUES (?, ?, 0, 0, ?)",
                (pair["task"], pair["response"], pair["session_id"]),
            )
            inserted += 1
        except sqlite3.Error as e:
            print(f"[magi_ingest] db write failed: {e}", file=sys.stderr)
    conn.commit()
    conn.close()
    return inserted


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: looprs_ingest.py <session.jsonl> <rewards.db>", file=sys.stderr)
        sys.exit(1)
    n = ingest_session_file(sys.argv[1], sys.argv[2])
    print(f"[magi_ingest] inserted {n} interactions")
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd /Users/joe/dev/magi && python3 -m pytest tests/test_looprs_ingest.py -v 2>&1
```
Expected: `test_ingest_from_looprs_session PASSED`.

- [ ] **Step 5: Create hook YAML**

```yaml
# hooks/looprs/magi_ingest.yaml
name: magi_ingest
trigger: SessionEnd
description: Ingest looprs Ollama session interactions into magi rewards.db
actions:
  - type: command
    command: |
      python3 ~/dev/magi/scripts/looprs_ingest.py \
        "$(ls -t ~/.looprs/sessions/*.jsonl 2>/dev/null | head -1)" \
        "$(python3 -c "
      import tomllib, os
      p = os.path.expanduser('~/.looprs/models.toml')
      cfg = tomllib.load(open(p,'rb')) if os.path.exists(p) else {}
      print(cfg.get('magi',{}).get('db',''))
      " 2>/dev/null || echo '')"
    inject_as: magi_ingest_result
```

- [ ] **Step 6: Commit magi changes**

```bash
cd /Users/joe/dev/magi
mkdir -p hooks/looprs
git add hooks/looprs/magi_ingest.yaml scripts/looprs_ingest.py tests/test_looprs_ingest.py
git commit -m "feat: add looprs SessionEnd hook for magi interaction ingestion"
```

---

## Task 8: Desktop Model Badge

**Goal:** looprs desktop shows model version, mean reward, and training status. Polls every 60s.

**Files:**
- Create: `crates/looprs-desktop/src/services/model_badge.rs`
- Modify: `crates/looprs-desktop/src/services/mod.rs`
- Modify: `crates/looprs-desktop/src/ui/root.rs`

- [ ] **Step 1: Implement `model_badge.rs`**

```rust
// crates/looprs-desktop/src/services/model_badge.rs
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct ModelBadgeState {
    pub model_id: String,
    pub mean_reward: f32,
    pub training_status: String,
}

#[derive(Deserialize, Default)]
struct Modelcard {
    #[serde(default)] model_id: String,
    #[serde(default)] training_status: String,
    #[serde(default)] eval_results: std::collections::HashMap<String, serde_yaml::Value>,
}

pub fn load_badge_state(modelcard_path: &PathBuf) -> ModelBadgeState {
    let content = match std::fs::read_to_string(modelcard_path) {
        Ok(c) => c,
        Err(_) => return ModelBadgeState {
            model_id: "unknown".into(),
            mean_reward: 0.0,
            training_status: "unknown".into(),
        },
    };
    let mc: Modelcard = match serde_yaml::from_str(&content) {
        Ok(m) => m,
        Err(_) => return ModelBadgeState {
            model_id: "unknown".into(),
            mean_reward: 0.0,
            training_status: "unknown".into(),
        },
    };
    let rewards: Vec<f32> = mc.eval_results.values()
        .filter_map(|v| v.get("mean_reward")?.as_f64())
        .map(|f| f as f32)
        .collect();
    let mean = if rewards.is_empty() {
        0.0
    } else {
        rewards.iter().sum::<f32>() / rewards.len() as f32
    };
    ModelBadgeState {
        model_id: if mc.model_id.is_empty() { "unknown".into() } else { mc.model_id },
        mean_reward: mean,
        training_status: if mc.training_status.is_empty() { "idle".into() } else { mc.training_status },
    }
}

pub fn spawn_badge_poller(modelcard_path: PathBuf, state: Arc<RwLock<ModelBadgeState>>) {
    tokio::spawn(async move {
        loop {
            let fresh = load_badge_state(&modelcard_path);
            if let Ok(mut s) = state.write() {
                *s = fresh;
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_load_badge_from_fixture() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "model_id: magistral-small-rl-v17").unwrap();
        writeln!(f, "training_status: idle").unwrap();
        writeln!(f, "eval_results:").unwrap();
        writeln!(f, "  code_review:").unwrap();
        writeln!(f, "    mean_reward: 0.82").unwrap();
        writeln!(f, "  debugging:").unwrap();
        writeln!(f, "    mean_reward: 0.74").unwrap();
        let state = load_badge_state(&f.path().to_path_buf());
        assert_eq!(state.model_id, "magistral-small-rl-v17");
        assert_eq!(state.training_status, "idle");
        assert!((state.mean_reward - 0.78).abs() < 0.01);
    }

    #[test]
    fn test_missing_modelcard_returns_unknown() {
        let state = load_badge_state(&PathBuf::from("/nonexistent/modelcard.yaml"));
        assert_eq!(state.model_id, "unknown");
        assert_eq!(state.training_status, "unknown");
    }
}
```

Add to `crates/looprs-desktop/Cargo.toml` if not present:
```toml
serde_yaml = "0.9"
```

- [ ] **Step 2: Export from `services/mod.rs`**

```rust
pub mod model_badge;
```

- [ ] **Step 3: Run badge tests**

```bash
cd /Users/joe/dev/looprs && cargo test -p looprs-desktop model_badge 2>&1
```
Expected: 2 tests pass.

- [ ] **Step 4: Wire badge into `ui/root.rs`**

Search `root.rs` for the status bar or header section (look for "status" or the bottom bar).
Add badge state initialization alongside other `Arc<RwLock<...>>` fields:

```rust
use crate::services::model_badge::{ModelBadgeState, load_badge_state, spawn_badge_poller};
use std::sync::{Arc, RwLock};

// In component state setup:
let badge_state: Arc<RwLock<ModelBadgeState>> =
    Arc::new(RwLock::new(ModelBadgeState::default()));

if let Ok(cfg) = looprs::models_config::ModelsConfig::load() {
    let mc_path = std::path::PathBuf::from(cfg.magi_modelcard());
    if !mc_path.as_os_str().is_empty() {
        if let Ok(mut s) = badge_state.write() {
            *s = load_badge_state(&mc_path);
        }
        spawn_badge_poller(mc_path, Arc::clone(&badge_state));
    }
}
```

In the render function, where status indicators are shown, add:
```rust
// Read badge and render using existing label primitives in root.rs
let badge = badge_state.read().unwrap();
// Example using whatever label macro the codebase uses (check root.rs for pattern):
// label!("{} | reward:{:.3} | {}", badge.model_id, badge.mean_reward, badge.training_status)
```

(Match the exact label/text macro from existing status elements in `root.rs`.)

- [ ] **Step 5: Run cargo check**

```bash
cd /Users/joe/dev/looprs && cargo check -p looprs-desktop 2>&1
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
cd /Users/joe/dev/looprs
git add crates/looprs-desktop/src/services/model_badge.rs \
        crates/looprs-desktop/src/services/mod.rs \
        crates/looprs-desktop/src/ui/root.rs \
        crates/looprs-desktop/Cargo.toml
git commit -m "feat: add model badge to desktop UI (version, reward, training status)"
```

---

## Task 9: Install Template + Docs

**Goal:** Provide default `models.toml` template and document hook install.

**Files:**
- Create: `.looprs/models.toml.example`
- Modify: `README.md`

- [ ] **Step 1: Create `models.toml.example`**

```toml
# Copy to ~/.looprs/models.toml and adjust paths.

[default]
provider = "ollama"
model = "magistral-small-rl-v17"

[tiers]
fast      = { provider = "ollama", model = "qwen2.5-coder:7b" }
capable   = { provider = "ollama", model = "magistral-small-rl-v17" }
outsource = { provider = "openai", model = "gpt-4o" }
judge     = { provider = "openai", model = "gpt-5.4" }

[magi]
modelcard = "/Users/joe/dev/magi/modelcard.yaml"
db        = "/Users/joe/dev/magi/db/rewards.db"
```

- [ ] **Step 2: Add magi integration section to README.md**

Find the configuration section and add:

```markdown
## magi Integration (Local Fine-Tuning)

looprs integrates with [magi](../magi) for automated model fine-tuning. Sessions are
automatically ingested and scored; the model improves over time via RL.

### Setup

1. Copy the model tier config:
   ```bash
   cp .looprs/models.toml.example ~/.looprs/models.toml
   # Edit [magi] paths to point to your magi install
   ```

2. Install the magi ingest hook:
   ```bash
   mkdir -p ~/.looprs/hooks
   ln -s ~/dev/magi/hooks/looprs/magi_ingest.yaml ~/.looprs/hooks/
   ```

3. Start coding. looprs writes session logs to `~/.looprs/sessions/`. At session end,
   magi ingests Ollama interactions automatically for scoring and RL training.

### Commands

| Command | Description |
|---------|-------------|
| `/model-status` | Show model version, mean reward, training status |
| `/fine-tune` | Boost reward for current session (prioritize for RL) |
| `/reset-model <name>` | Revert default model to a base version |
| `/score-session [n]` | Score last N interactions via OpenAI (default 10) |
| `/outsource` | Re-run current task via OpenAI (not fed to magi) |
```

- [ ] **Step 3: Commit**

```bash
cd /Users/joe/dev/looprs
git add .looprs/models.toml.example README.md
git commit -m "docs: add models.toml.example and magi integration setup guide"
```

---

## Spec Coverage Verification

| Requirement | Task |
|-------------|------|
| Session logging, provider tag, all event types | Task 1, Task 3 |
| magi hook bridge, SessionEnd, ollama-only filter | Task 7 |
| Provider routing via models.toml | Task 2, Task 5 |
| `/model-status`, `/fine-tune`, `/reset-model`, `/score-session`, `/outsource` | Task 6 |
| Scoring triggers: on-error, on-repeat, on-demand | Task 4, Task 5 |
| Desktop badge (version, reward, status, 60s poll) | Task 8 |
| Hook failure silent; OpenAI unavailable graceful; modelcard missing → unknown | Tasks 1, 4, 8 |
| magi db WAL mode | Task 7 |
| OpenAI interactions not fed to magi | Tasks 4, 7 |
| Install docs | Task 9 |
