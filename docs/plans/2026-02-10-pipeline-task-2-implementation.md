# Pipeline Task 2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add minimal pipeline types and a JSONL pipeline event logger with a passing test.

**Architecture:** Introduce a new `pipeline` module with a small `types` module for structs, a `logging` module that appends JSON objects as lines to `events.jsonl`, and a `mod.rs` that re-exports items plus a placeholder `PipelineRunner`. Logging uses unix millis timestamps and optional `run_id` without new dependencies.

**Tech Stack:** Rust, serde/serde_json, std::fs I/O, tempfile for tests.

---

### Task 2: Add pipeline types and JSONL event logging

**Files:**
- Create: `src/pipeline/mod.rs`
- Create: `src/pipeline/types.rs`
- Create: `src/pipeline/logging.rs`
- Modify: `src/lib.rs`
- Test: `src/pipeline/logging.rs`

**Step 1: Write the failing test**

Add test in `src/pipeline/logging.rs`:

```rust
#[test]
fn test_jsonl_event_written() {
    let dir = tempfile::tempdir().unwrap();
    let logger = PipelineLogger::new(dir.path().to_path_buf()).unwrap();
    logger.log_event("test", serde_json::json!({"ok": true})).unwrap();
    let entries = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
    assert!(entries.contains("\"step\":\"test\""));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_jsonl_event_written --lib`
Expected: FAIL with missing `PipelineLogger` or module errors.

**Step 3: Write minimal implementation**

- `src/pipeline/types.rs`: add `PipelineContext`, `StepResult`, `PipelineReport`, `ToolResult`, `RewardReport` structs/enums minimally defined for now (derive `Debug`, `Clone`, `Serialize`, `Deserialize` as needed).
- `src/pipeline/logging.rs`: implement `PipelineLogger` that writes JSONL to `events.jsonl` under `log_dir`. Include fields `step`, `data`, optional `ts` (unix millis) and optional `run_id`.
- `src/pipeline/mod.rs`: export `types` and `logging`, and add a `PipelineRunner` placeholder (empty struct for now) with basic `new` or `default` if needed.
- `src/lib.rs`: `pub mod pipeline;` and re-exports if required.

Minimal logger behavior example:

```rust
pub struct PipelineLogger { log_dir: PathBuf, run_id: Option<String> }

impl PipelineLogger {
    pub fn new(log_dir: PathBuf) -> std::io::Result<Self> { /* create dir */ }
    pub fn log_event(&self, step: &str, data: serde_json::Value) -> std::io::Result<()> {
        // build JSON object with step/data/ts/run_id, append to events.jsonl
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_jsonl_event_written --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/mod.rs src/pipeline/types.rs src/pipeline/logging.rs src/lib.rs
git commit -m "feat: add pipeline types and jsonl logging"
```
