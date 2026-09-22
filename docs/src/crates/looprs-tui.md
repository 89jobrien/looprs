# looprs-tui

Interactive terminal UI surfaces built on `ratatui`/`crossterm`, consumed by `looprs-cli`.

## Purpose

Small, self-contained terminal interactions that don't belong in the line-based REPL:
a single-select menu and a full-frame chat view. Depends on `looprs` for `Agent`; its
render-only transcript message type remains private to the TUI.

## Public API

| Export | Description |
|--------|-------------|
| `select(title, items) -> Result<Option<usize>>` | Render and drive an interactive single-select menu; `None` on cancel (Esc/q) |
| `chat::run(agent: Agent) -> Result<()>` | Run the full chat TUI loop against a bootstrapped agent; blocks until Esc |
| `output::ChannelOutput` | `UserOutput` adapter that forwards events over an `mpsc` channel, so `chat::run` can render streamed text inside its own frame |

## CLI subcommands

Wired in `looprs-cli`'s `main.rs`:

| Subcommand | Behavior |
|------------|----------|
| `looprs provider` | Calls `select()` twice (provider, then model for `local`), writes the choice to `.looprs/provider.json` |
| `looprs tui` | Bootstraps an `Agent`, swaps in `ChannelOutput`, and runs `chat::run()` |

## Behavior and testing

The provider selector clears and redraws between chained menus. The chat transcript
retains submitted user messages and turn errors, ignores new submissions while a turn is
running, and restores the terminal when it exits.

Unit tests cover key handling, rendering state, and output-channel behavior. A stable
process-level PTY harness is still pending because terminal readiness currently races with
input injection. See `.claude/skills/run-looprs/SKILL.md` for the tmux-based interactive
driver used for manual end-to-end checks.
