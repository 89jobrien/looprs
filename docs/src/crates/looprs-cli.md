# looprs-cli

The binary crate. Owns CLI argument parsing, the interactive REPL, and the runtime
facade that wires `looprs` crate components into a runnable program.

## Purpose

`looprs-cli` is the thin surface layer between the operating system and the `looprs`
runtime. It parses arguments, sets up the provider and agent, and either runs a single
scriptable turn or drops into the interactive REPL.

## Binary

Installed as `looprs`. Build and install:

```bash
cargo build --release
# binary at target/release/looprs

cargo install --path crates/looprs-cli
# binary at ~/.cargo/bin/looprs
```

## CLI Arguments

Parsed by `looprs_cli::args::CliArgs`:

| Flag | Short | Description |
|------|-------|-------------|
| `--prompt <text>` | `-p` | Run a single turn with this prompt (non-interactive) |
| `--file <path>` | `-f` | Read prompt from file, run single turn (non-interactive) |
| `--model <id>` | `-m` | Override the model for this session |
| `--quiet` | `-q` | Suppress status output |
| `--no-hooks` | | Disable lifecycle hooks for this session |
| `--json` | | Output responses as JSON |
| `--machine-log` | | Write structured JSONL machine log |
| `--machine-protocol <version>` | | Emit versioned machine envelopes; currently `looprs-machine/v1` |
| `--run-id <id>` | | Set the stable identifier included in machine events |
| `--deadline-seconds <n>` | | Cancel the active run after a positive timeout |
| `--cancel-file <path>` | | Cancel the active run when the path exists |
| `--help` | `-h` | Print usage and exit successfully |
| `--version` | `-V` | Print the package version and exit successfully |

When neither `--prompt` nor `--file` is passed, the binary enters interactive REPL mode.
When either is passed, `is_scriptable()` returns true and the binary runs one turn and
exits.

`--file` takes precedence over `--prompt` in `get_prompt()` — if both are provided, the
file is read and the inline prompt is ignored.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `seed [DIR]` | Write example config files to `DIR` (default `.looprs`); does not overwrite |
| `provider` | Interactive menu (via `looprs-tui`) to pick provider/model, written to `.looprs/provider.json` |
| `tui` | Alternate chat TUI (via `looprs-tui`): scrollback transcript over an input box, instead of the REPL |

`provider` and `tui` are implemented in `crates/looprs-tui` and pulled in as a dependency.

## Machine protocol

`--machine-log` emits the legacy `{kind,data}` JSONL records on stderr. Selecting
`--machine-protocol looprs-machine/v1` emits versioned stderr envelopes containing the
protocol, run ID, monotonically increasing sequence, timestamp, and event. Human and
assistant output remains on stdout.

Versioned runs emit one terminal lifecycle event: `run.succeeded`, `run.failed`, or
`run.cancelled`. `--run-id`, `--deadline-seconds`, and `--cancel-file` require the
versioned protocol; deadlines and cancellation files can interrupt an in-flight provider
request.

## Modules

| Module | Contents |
|--------|----------|
| `args` | `CliArgs` struct and parser |
| `cli` | Top-level run logic; wires provider, agent, and mode |
| `repl` | Interactive REPL powered by `rustyline`; handles line editing, history, completions |
| `runtime/` | Runtime facade types |
| `main.rs` | Entrypoint; calls `cli::run()` |

## REPL

The interactive REPL uses `rustyline` for line editing and history. Pure logic functions
(fuzzy scoring, completion hints, best-match) are unit and property tested in `repl.rs`.
The rustyline interactive layer itself is not covered by automated PTY tests.

## Relationship to `looprs`

`looprs-cli` depends on `looprs` for the `Agent`, registries, providers, and all runtime
behaviour. `looprs-cli` adds no business logic — if it does more than parse, wire, and
dispatch, that logic belongs in `looprs` instead.
