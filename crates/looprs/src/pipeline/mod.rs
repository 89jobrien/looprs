pub mod context_compact;
pub mod logging;
pub mod types;

// IDEA(M1) / TODO(pipeline-activation): activate the self-improvement pipeline (idea #10).
//
// `PipelineConfig.enabled` is false and `PipelineChecksConfig` has all flags
// false. Steps to activate:
//
//   1. Define a concrete default check set in `PipelineChecksConfig::default()`:
//        run_lint = true, run_tests = true (cargo nextest --workspace).
//   2. Implement `PipelineRunner::run_checks(&self, cfg: &PipelineChecksConfig)`
//      that shells out to each enabled check and collects pass/fail results.
//   3. In `Agent::run_turn()`, after a successful tool-use round-trip, call
//      `PipelineRunner::run_checks` if `config.pipeline.enabled`. Surface
//      failures as `AgentError::PipelineFailure(String)`.
//   4. Wire `auto_revert`: if checks fail and `auto_revert = true`, restore the
//      session to the pre-turn message state and re-prompt the agent.
//   5. Flip `PipelineConfig::default().enabled = true` once steps 1-4 are stable.

#[derive(Debug, Default)]
pub struct PipelineRunner;
