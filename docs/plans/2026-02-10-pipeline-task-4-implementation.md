# Pipeline Task 4 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement unified diff application and changed-file detection for the pipeline.

**Architecture:** Add a small pipeline module that shells out to `git apply` for patching and `git diff --name-only` to capture changed files, returning a simple result struct. Keep it minimal and deterministic with a single test.

**Tech Stack:** Rust, git CLI, tempfile.

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
