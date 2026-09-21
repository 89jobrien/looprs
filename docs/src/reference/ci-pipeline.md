# CI Pipeline

Run the full pipeline:

```sh
cargo xtask check ci
```

`cargo xtask ci` (without `check`) does **not** work — `taskit` has no bare `ci`
subcommand; it's `taskit check ci`, and `xtask` just passes args through.

## Steps

Defined in `Cruxfile` (`pipeline: looprs-ci`):

| Step | Command | Gate |
|------|---------|------|
| Self-check | `taskit self check` | Yes |
| Format check | `taskit check fmt --check` | No |
| Lint | `taskit check lint` | No |
| Compile tests | `taskit check compile` | No |
| Test | `taskit test run` | No |
| Deps | `taskit check deps` | No |
| Drift | `taskit protocol drift` | No |

## Runtime Auto-Revert

When `pipeline.enabled`, `pipeline.block_on_failure`, and `pipeline.auto_revert` are all
enabled, looprs snapshots the Git worktree immediately before the turn's first tool
dispatch. A blocking pipeline failure restores the pre-turn conversation, index,
tracked files, and non-ignored untracked files. This preserves pre-existing staged,
unstaged, partially staged, and untracked changes while removing changes introduced
during the failed turn.

The transaction applies only to the Git worktree containing the tool working directory.
Ignored files, nested repositories, changes outside that worktree, and external side
effects are not restored. If the baseline cannot be captured, tool execution is stopped.
If rollback itself fails, the pipeline error reports that failure and leaves the worktree
for manual recovery rather than claiming the rollback succeeded.
