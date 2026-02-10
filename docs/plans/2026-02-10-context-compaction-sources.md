# Context Compaction Sources Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement pipeline context compaction that gathers diffs, recent files, and glob-matched sources with size limits.

**Architecture:** Add a new `context_compact` module with `compact_context` that shells out to git for changed files, uses `rg --files` (or a filesystem fallback) for glob matching and recency, reads files with size limits, and returns ordered text slices. Wire the module into `src/pipeline/mod.rs` and keep the implementation deterministic and minimal.

**Tech Stack:** Rust, std::process::Command, glob crate, tempfile (tests), git, rg (optional).

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
