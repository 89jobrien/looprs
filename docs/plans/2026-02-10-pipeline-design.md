# Pipeline Design (Agentic Loop Integration)

Date: 2026-02-10

## Goal

Integrate ideas from `agentic_pipeline` into the looprs core library as an opt-in, config-driven pipeline. The pipeline provides deterministic, tool-grounded runs with context compaction, diff-apply, checks, composite reward, auto-revert on failure, and JSONL event logging to `.looprs/agent_logs/`.

## Approach (Recommended)

Create a minimal core orchestrator module under `src/pipeline/` and wire it into the agent lifecycle when `pipeline.enabled=true`. Keep the pipeline self-contained and testable, and keep the existing hook system as an observer, not a dependency.

## Architecture

- New module: `src/pipeline/`
  - `PipelineRunner::run(...)` entry point
  - Step helpers: context compaction, diff-apply, checks, reward, revert, logging
  - Types: `PipelineContext`, `StepResult`, `PipelineReport`
- Config: `app_config.rs` and `.looprs/config.json`
  - `pipeline.enabled` (default false)
  - `pipeline.log_dir` (default `.looprs/agent_logs/`)
  - `pipeline.reward_threshold` (for gating)
  - `pipeline.auto_revert` (true)
  - `pipeline.fail_fast` / `pipeline.block_on_failure`
  - `pipeline.require_tools` (false)
  - `pipeline.compaction` (diff+recent, top-K relevance, include globs)
  - `pipeline.checks` (build/test/lint/type/bench)

## Data Flow

1) Agent builds `PipelineContext` (repo root, git status, recent files, optional diff path, tool availability).
2) Context compaction uses up to three sources, each toggled independently:
   - A) git diff + recent files
   - B) top-K relevant files (rg/heuristics)
   - C) include globs
   Produces ordered `ContextSlice` entries, size-bounded, and returns a compacted text block for prompt injection.
3) Optional diff-apply; best-effort changed file detection; failure recorded.
4) Checks: build/test/lint/type/bench, returning rc/stdout/stderr/timeouts. Missing tools yield `null` unless `require_tools=true`.
5) Composite reward: score weighted by tool outcomes. If below threshold, mark run failed.
6) Auto-revert: on failure, revert working tree changes only (no log deletion).
7) JSONL event logging to `.looprs/agent_logs/`, one event per step with run_id and payload.

## Error Handling

- Each step returns a `StepResult` with `ok`, `error`, and `artifacts`.
- The runner never panics; it returns `PipelineReport` with `overall_ok` and `failure_reason`.
- Internal errors are logged and returned as failed report without leaving partial state.

## Testing

- Unit tests for context compaction ordering, size bounds, and toggle behavior.
- Unit tests for diff-apply and changed-file detection with and without `unidiff`.
- Unit tests for reward computation and threshold gating.
- Integration tests for runner behavior and JSONL output schema.
- Integration test for auto-revert affecting only working tree files.
- Agent integration test: compact context injected; failure triggers revert; logs present.

## Rollout

- Opt-in by config only; default disabled.
- Add example config and minimal docs in `README.md`.
- No breaking changes.
