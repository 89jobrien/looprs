# Context Compaction Glob Guard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent include_globs from escaping repo_root and add deterministic tests for glob escaping and git diff/recent inclusion.

**Architecture:** Add a repo_root boundary check to globbing so matches outside are filtered out (or reject absolute patterns), and extend existing tests in the context compaction module to cover absolute glob escape and git diff/recent inclusion behavior. Keep behavior deterministic by controlling temp repos and git state.

**Tech Stack:** Rust, existing test utilities in `src/pipeline/context_compact.rs`, `git` CLI in tests.

---

### Task 1: Add failing test for glob escape (absolute glob outside repo_root)

**Files:**
- Modify: `src/pipeline/context_compact.rs`

**Step 1: Write the failing test**

Add a new test in the existing test module:
- Create a temp repo root with one file inside.
- Create a separate temp dir/file outside that root.
- Provide an absolute glob targeting the outside file (e.g., `/tmp/...` from the temp dir path).
- Call `compact_context` with `include_globs` set to that absolute glob.
- Assert that the output does NOT contain the outside file path or content.

**Step 2: Run test to verify it fails**

Run: `cargo test context_compact -- --nocapture`
Expected: FAIL because output still includes the outside file path/content.

**Step 3: Write minimal implementation**

Implement repo_root boundary filtering for include_globs results (see Task 2). Keep behavior unchanged for valid in-repo matches.

**Step 4: Run test to verify it passes**

Run: `cargo test context_compact -- --nocapture`
Expected: PASS.

**Step 5: Commit**

(Commit after Task 2 to keep changes together.)

---

### Task 2: Add failing test for git diff/recent inclusion in a temp repo

**Files:**
- Modify: `src/pipeline/context_compact.rs`

**Step 1: Write the failing test**

Add a new test:
- Initialize a git repo in a temp dir.
- Create and commit a file.
- Modify the file (unstaged).
- Call `compact_context` with `include_diff = true` and `include_recent = true`.
- Assert the output includes the file path (and/or contents if that is current behavior).

**Step 2: Run test to verify it fails**

Run: `cargo test context_compact -- --nocapture`
Expected: FAIL if current behavior doesn’t include modified file paths in a git repo.

**Step 3: Write minimal implementation**

Adjust diff/recent logic only if needed to ensure git-initialized temp repo behaves as expected, keeping existing behavior unchanged elsewhere.

**Step 4: Run test to verify it passes**

Run: `cargo test context_compact -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/pipeline/context_compact.rs
git commit -m "fix: constrain pipeline compaction globs"
```

---

### Task 3: Verification

**Step 1: Run focused tests**

Run: `cargo test context_compact -- --nocapture`
Expected: PASS.

**Step 2: Optional broader check**

Run: `cargo test`
Expected: PASS.
