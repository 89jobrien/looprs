# Project Structure Modernization Design

**Goal:** Restructure the crate for modern Rust best practices, improved testability, and clearer module boundaries while preserving behavior.

**Architecture:** Convert to a library-first layout with a thin CLI binary. Core logic moves into `src/lib.rs` and internal modules; CLI remains a small wrapper. Tools are split into submodules for clarity and targeted testing.

**Tech Stack:** Rust 2024 edition, `anyhow`, `thiserror`, `tokio`, `reqwest`, `serde`.

---

## Approach Options Considered

1. **Library-first split (recommended)**
   - Move core logic into `src/lib.rs`; expose a minimal public API.
   - Keep CLI in `src/bin/looprs.rs` (or a thin `src/main.rs`).
   - Best balance of reuse and minimal churn.

2. **Module-only refactor**
   - Split modules without introducing a library API.
   - Simpler change set but weaker testability and reuse.

3. **Workspace split**
   - Separate `looprs-core` and `looprs-cli` crates.
   - Most scalable but heavier refactor; likely overkill now.

## Chosen Design

### Public API
- `src/lib.rs` will expose only:
  - `pub use crate::agent::Agent;`
  - `pub use crate::config::ApiConfig;`
- All other modules remain internal (module privacy by default).

### File Layout
- `src/lib.rs` — public API boundary.
- `src/bin/looprs.rs` (or thin `src/main.rs`) — CLI entry.
- `src/agent.rs`, `src/api.rs`, `src/config.rs`, `src/cli.rs` — behavior preserved.
- `src/tools/mod.rs` — dispatcher + `ToolContext` + tool registry.
- `src/tools/{read,write,edit,glob,grep,bash}.rs` — single-responsibility tool files.
- `src/tools/error.rs` — `ToolError` type.

### Error Handling
- Continue using `anyhow::Result` at public API boundaries.
- Keep `ToolError` typed internally for tool failures.
- No behavioral changes; refactor only for structure and testability.

### Testing (TDD)
- Unit tests:
  - `cli::parse_input` tests in `src/cli.rs`.
  - Tool tests in each tool module using temp files/directories.
- Integration tests:
  - `tests/cli_smoke.rs` for minimal, offline CLI parsing checks.
- Avoid network calls and nondeterministic behavior in tests.

### Cargo Metadata
Add missing metadata to `Cargo.toml`:
- `description`, `license`, `repository`, `readme`, `rust-version`.

## Success Criteria
- Crate builds and tests pass.
- CLI behavior unchanged.
- Clear separation between library logic and CLI entrypoint.
- Tool implementations are isolated and individually testable.
- `Cargo.toml` provides standard metadata for packaging.
