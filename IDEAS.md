# looprs — Feature Ideas

Generated 2026-07-12 from codebase scan. Every item traces to an observed gap,
incomplete feature, or architectural seam in the repo.

---

## Quick Wins (< 1 day)

### 1. REPL pure-function property tests
`fuzzy_score`, `best_match`, `completion_hint` are untested. `proptest` is already
in `Cargo.toml`. A handful of invariant-based tests (monotonicity, symmetry,
no-empty-match) would close a known gap documented in the test-coverage plan.

### 2. Streaming output
The agent loop buffers full responses before printing. `claudius` and `async-openai`
both support streaming. A streaming path would make long responses feel responsive —
one-line UX difference but high perceived value. The `UserOutput` port (Phase 1 of
the hex refactor) is the right abstraction to hang this on.

### 3. Active `.looprs/agents/` definitions
The agents directory exists with only a README. The registry is fully wired. Adding
2-3 real agent definitions (e.g. `reviewer`, `planner`, `debugger`) would make
delegation usable and validate the delegation loop end-to-end.

---

## Medium (1–3 days)

### 4. Provider conformance test suite
No contract tests exist for `InferenceProvider` implementations — documented gap. A
shared test matrix (correct tool-use round-trip, retry on 429, model name
normalisation, timeout propagation) run against each provider would catch drift. CI
already runs live-test opt-in via `LOOPRS_RUN_LIVE_LLM_TESTS=1`.

### 5. Pipeline feature activation
`config.json` has `pipeline.enabled = false` and `pipeline.checks` is an empty list.
The self-improvement loop is half-wired. Defining a concrete check set (clippy,
nextest, coverage threshold) and enabling it would close a visible gap the config
already acknowledges.

### 6. `PluginExecutor` port integration
The port is defined in `looprs-core` but `run_turn` calls `execute_tool()` the free
function directly. Routing tool dispatch through `PluginExecutor` would complete Phase
2 of the hex refactor and make tool interception (logging, sandboxing, mocking)
possible without touching agent internals.

### 7. Context compaction / windowing
The pipeline config mentions `compaction` but it is unimplemented. Long sessions
accumulate unbounded history. A simple sliding-window or summary-injection strategy
(configurable `max_context_tokens`) would prevent silent token-limit failures.

---

## Large (> 3 days)

### 8. Hexagonal refactor Phase 1–4 completion
The plan exists in `docs/hexagonal-refactor.md`. Phases 1–4 extract `UserOutput`,
`ToolExecutor`, `SessionStore`, and introduce `AgentServices`. Currently `Agent` calls
`ui::*` statics, `execute_tool()` free function, and constructs `SessionLogger`
directly. Completing the refactor makes the runtime fully testable without a real
provider or filesystem.

### 9. Persistent observation layer
`ObservationManager` is in-memory only. Traces write JSONL to
`.looprs/observability/` but manager state is lost on exit. A `SessionStore` port
backed by SQLite (already a dependency via `rusqlite`) would enable cross-session
analytics, session replay, and cost tracking.

### 10. MCP tool support
The tool system has 7 built-in tools. No MCP client integration exists. Adding an MCP
adapter (`ToolExecutor` port that delegates to an MCP server) would make looprs
composable with the broader MCP ecosystem — high leverage given the codebase is
already a tool-execution runtime.
