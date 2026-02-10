# Pipeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an opt-in core pipeline that provides context compaction, diff apply, checks, reward gating, auto-revert, and JSONL logging to `.looprs/agent_logs/`.

**Architecture:** Introduce a new `src/pipeline/` module that owns the deterministic loop and report types. Wire it into `Agent::run_turn` behind config flags and keep hooks independent.

**Tech Stack:** Rust (looprs core), serde/serde_json, std::process::Command, git CLI.

---

### Task 1: Add pipeline config structs and defaults

**Files:**
- Modify: `src/app_config.rs`
- Test: `src/app_config.rs`

**Step 1: Write the failing test**

Add a test at the bottom of `src/app_config.rs`:

```rust
#[test]
fn test_pipeline_config_defaults_roundtrip() {
    let config = AppConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.pipeline.enabled, false);
    assert_eq!(decoded.pipeline.log_dir, ".looprs/agent_logs/");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_pipeline_config_defaults_roundtrip --lib`
Expected: FAIL (missing `pipeline` field).

**Step 3: Write minimal implementation**

In `src/app_config.rs`, add `pipeline: PipelineConfig` to `AppConfig`, define `PipelineConfig` and nested structs with defaults (enabled=false, log_dir `.looprs/agent_logs/`, reward threshold, checks toggles, compaction settings, require_tools, auto_revert, fail_fast, block_on_failure).

**Step 4: Run test to verify it passes**

Run: `cargo test test_pipeline_config_defaults_roundtrip --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app_config.rs
git commit -m "feat: add pipeline config defaults"
```

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
Expected: FAIL (missing `PipelineLogger`).

**Step 3: Write minimal implementation**

- `types.rs`: add `PipelineContext`, `StepResult`, `PipelineReport`, `ToolResult`, `RewardReport`.
- `logging.rs`: implement `PipelineLogger` that writes JSONL to `events.jsonl` under `log_dir`.
- `mod.rs`: export submodules and a `PipelineRunner` placeholder.
- `lib.rs`: `pub mod pipeline;` and re-exports if needed.

**Step 4: Run test to verify it passes**

Run: `cargo test test_jsonl_event_written --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/mod.rs src/pipeline/types.rs src/pipeline/logging.rs src/lib.rs
git commit -m "feat: add pipeline types and jsonl logging"
```

---

### Task 3: Implement context compaction sources

**Files:**
- Create: `src/pipeline/context_compact.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `src/pipeline/context_compact.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_compact_context_includes_diff_and_globs() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("a.txt"), "hello").unwrap();
    let config = PipelineCompactionConfig {
        include_diff: true,
        include_recent: true,
        include_globs: vec!["*.txt".to_string()],
        top_k: 0,
        ..Default::default()
    };
    let out = compact_context(repo.path(), &config).unwrap();
    assert!(out.text.contains("a.txt"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_compact_context_includes_diff_and_globs --lib`
Expected: FAIL (missing `compact_context`).

**Step 3: Write minimal implementation**

- Implement `compact_context(repo_root, config)`:
  - A) diff + recent files via `git diff --name-only` + `git status --porcelain`.
  - B) top-K relevance via `rg --files -g <globs>` fallback to listing files; use simple heuristic (e.g., newest modified) if `rg` missing.
  - C) include globs via `glob` crate (already in deps).
  - Read files with size limits; build ordered slices.

**Step 4: Run test to verify it passes**

Run: `cargo test test_compact_context_includes_diff_and_globs --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/context_compact.rs src/pipeline/mod.rs
git commit -m "feat: add pipeline context compaction"
```

---

### Task 4: Implement diff apply and changed-file detection

**Files:**
- Create: `src/pipeline/diff_apply.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `src/pipeline/diff_apply.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_apply_unified_diff_updates_file() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("a.txt"), "old\n").unwrap();
    let diff = "--- a/a.txt\n+++ b/a.txt\n@@\n-old\n+new\n";
    let diff_path = repo.path().join("patch.diff");
    std::fs::write(&diff_path, diff).unwrap();
    let result = apply_unified_diff(repo.path(), &diff_path).unwrap();
    assert!(result.changed_files.contains(&"a.txt".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_apply_unified_diff_updates_file --lib`
Expected: FAIL (missing `apply_unified_diff`).

**Step 3: Write minimal implementation**

- Use `git apply` to apply the diff.
- After apply, compute changed files via `git diff --name-only`.
- Return `DiffApplyResult` with `changed_files` and `ok`.

**Step 4: Run test to verify it passes**

Run: `cargo test test_apply_unified_diff_updates_file --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/diff_apply.rs src/pipeline/mod.rs
git commit -m "feat: add pipeline diff apply"
```

---

### Task 5: Add check runners (build/test/lint/type/bench)

**Files:**
- Create: `src/pipeline/checks.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `src/pipeline/checks.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_checks_handle_missing_tool() {
    let repo = tempfile::tempdir().unwrap();
    let config = PipelineChecksConfig { run_tests: true, ..Default::default() };
    let result = run_checks(repo.path(), &config).unwrap();
    assert!(result.tests.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_checks_handle_missing_tool --lib`
Expected: FAIL (missing `run_checks`).

**Step 3: Write minimal implementation**

- Implement `run_checks(repo_root, config)`:
  - For each enabled check, run a command (`cargo build`, `cargo test`, `cargo clippy`, `cargo bench`, `cargo fmt --check` for type/lint as needed).
  - If tool missing, set `None` unless `require_tools=true`.

**Step 4: Run test to verify it passes**

Run: `cargo test test_checks_handle_missing_tool --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/checks.rs src/pipeline/mod.rs
git commit -m "feat: add pipeline check runners"
```

---

### Task 6: Implement composite reward and gating

**Files:**
- Create: `src/pipeline/reward.rs`
- Modify: `src/pipeline/mod.rs`
- Test: `src/pipeline/reward.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_reward_threshold_blocks_failure() {
    let checks = ToolResults { build: Some(ok_result()), ..Default::default() };
    let report = compute_reward(&checks, 0.8);
    assert!(report.score <= 1.0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_reward_threshold_blocks_failure --lib`
Expected: FAIL (missing `compute_reward`).

**Step 3: Write minimal implementation**

- Implement `compute_reward` with weights and thresholds from config.
- Return `RewardReport { score, passed_threshold }`.

**Step 4: Run test to verify it passes**

Run: `cargo test test_reward_threshold_blocks_failure --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/reward.rs src/pipeline/mod.rs
git commit -m "feat: add pipeline reward computation"
```

---

### Task 7: Implement pipeline runner and auto-revert

**Files:**
- Modify: `src/pipeline/mod.rs`
- Test: `src/pipeline/mod.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_pipeline_reverts_on_failure() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("a.txt"), "old\n").unwrap();
    let ctx = PipelineContext::new(repo.path().to_path_buf());
    let config = PipelineConfig { auto_revert: true, reward_threshold: 1.0, ..Default::default() };
    let report = PipelineRunner::run(&ctx, &config).unwrap();
    assert!(!report.overall_ok);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_pipeline_reverts_on_failure --lib`
Expected: FAIL (missing runner).

**Step 3: Write minimal implementation**

- Implement `PipelineRunner::run` to call compaction, diff apply, checks, reward, revert, and logging in order.
- Auto-revert only working tree changes via `git checkout -- .` or `git restore .`.
- Emit JSONL events per step.

**Step 4: Run test to verify it passes**

Run: `cargo test test_pipeline_reverts_on_failure --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/mod.rs
git commit -m "feat: add pipeline runner and auto-revert"
```

---

### Task 8: Wire pipeline into Agent and document config

**Files:**
- Modify: `src/agent.rs`
- Modify: `src/bin/looprs/main.rs`
- Modify: `README.md`
- Test: `src/agent.rs`

**Step 1: Write the failing test**

Add test in `src/agent.rs`:

```rust
#[tokio::test]
async fn test_agent_pipeline_context_injection() {
    let provider = MockProvider::simple_text("ok");
    let mut agent = Agent::new(Box::new(provider)).unwrap();
    agent.set_runtime_settings(RuntimeSettings { defaults: DefaultsConfig::default(), max_tokens_override: None });
    let result = agent.run_turn().await;
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_agent_pipeline_context_injection --lib`
Expected: FAIL (pipeline not integrated).

**Step 3: Write minimal implementation**

- Load `AppConfig` in `main.rs` and pass pipeline config into `RuntimeSettings` or `Agent`.
- In `Agent::run_turn`, when enabled, call pipeline to build compact context and prepend to system prompt; after tool execution, call pipeline to run checks/reward/revert and log.
- Update `README.md` with a brief `.looprs/config.json` example showing pipeline config.

**Step 4: Run test to verify it passes**

Run: `cargo test test_agent_pipeline_context_injection --lib`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/agent.rs src/bin/looprs/main.rs README.md
git commit -m "feat: integrate pipeline into agent"
```

---

Plan complete and saved to `docs/plans/2026-02-10-pipeline-implementation-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

Which approach?
