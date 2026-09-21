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
