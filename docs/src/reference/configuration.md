# Configuration

## taskit.toml

Development workflow configuration lives in `taskit.toml` at the workspace root. Runtime
settings live in `.looprs/config.json`, while provider selection lives in
`.looprs/provider.json`; user-level defaults use the corresponding files under
`$HOME/.looprs/`.

### Sections

| Section | Purpose |
|---------|---------|
| `[workspace]` | Crate list, propagation rules, offline skip |
| `[protocol]` | Contract surface drift detection |
| `[coverage]` | Coverage enforcement |
| `[ci]` | Pipeline steps |

`[workspace].crates` includes `looprs-macros`, `looprs-core`, `looprs`,
`looprs-cli`, and `looprs-tui`, so taskit's affected-crate detection covers every
publishable runtime crate. The excluded `fuzz` package is checked separately by nightly
automation.
