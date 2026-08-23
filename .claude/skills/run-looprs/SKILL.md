---
name: run-looprs
description: Build, launch, and drive the looprs CLI/TUI (agent loop runtime) — run a scriptable prompt, launch the looprs provider menu or looprs tui chat TUI, type/send keys, capture the pane, screenshot the terminal state. Use when asked to run looprs, test the TUI, verify a change to the agent loop, or drive the provider/tui subcommands.
---

All paths below are relative to the looprs workspace root (this repo's
top level, where `Cargo.toml` lists `crates/looprs-core`, `crates/looprs`,
`crates/looprs-cli`, `crates/looprs-tui`, `xtask`).

## Prerequisites

```
which cargo tmux    # both required; installed via mise/rustup + brew on this machine
```

For a real (non-erroring) end-to-end response without touching a paid
API, use the `local` provider — it shells out to a running `ollama`
daemon:

```
ollama list          # must show at least one model, e.g. functiongemma:latest
```

## Build

```
.claude/skills/run-looprs/driver.sh build
# -> cargo build -p looprs-cli; produces target/debug/looprs
```

## Run (agent path) — use `driver.sh`

The driver (`.claude/skills/run-looprs/driver.sh`, POSIX `sh`) has two
modes:

**1. Direct invocation** (`prompt`) — the scriptable, non-interactive
surface (`looprs -p "<text>"`). This is the path most PRs touching
`crates/looprs`'s agent/provider/hook logic actually exercise — no
terminal needed:

```
.claude/skills/run-looprs/driver.sh prompt "reply with the single word OK" local
# -> 📋 Loaded 3 project rule(s)
#    >> looprs | ollama/functiongemma:latest | /Users/joe/dev/looprs
#    ● OK
```

Second arg is the provider (`anthropic`, `openai`, `local`); defaults
to `anthropic`. **`anthropic` reliably fails in this environment**
(see Gotchas) — use `local` for a response you can actually verify,
or `openai` if `OPENAI_API_KEY` is set.

**2. tmux driver** — for the interactive surfaces added by
`crates/looprs-tui`: `looprs provider` (provider/model select menu)
and `looprs tui` (streaming chat TUI). All tmux sessions run on a
dedicated socket (`-L looprs_driver`) so they don't collide with your
own tmux.

```
.claude/skills/run-looprs/driver.sh tui-launch <session>       # launch `looprs tui`
.claude/skills/run-looprs/driver.sh provider-launch <session>  # launch `looprs provider`
.claude/skills/run-looprs/driver.sh type <session> "<text>"    # tmux send-keys -l (literal)
.claude/skills/run-looprs/driver.sh send <session> <key>       # tmux send-keys (key name: Enter, Down, Up, Esc, BSpace, j, k, q...)
.claude/skills/run-looprs/driver.sh capture <session>          # tmux capture-pane -p (the "screenshot")
.claude/skills/run-looprs/driver.sh stop <session>              # kill one session
.claude/skills/run-looprs/driver.sh stop-all                    # kill the whole driver socket
```

Verified chat round-trip:

```
.claude/skills/run-looprs/driver.sh tui-launch t1
# (capture once to let it settle — see Gotchas)
.claude/skills/run-looprs/driver.sh capture t1
.claude/skills/run-looprs/driver.sh type t1 "say OK"
.claude/skills/run-looprs/driver.sh send t1 Enter
.claude/skills/run-looprs/driver.sh capture t1
# -> ┌looprs chat (thinking...)─...  /  ┌waiting for response...─...
.claude/skills/run-looprs/driver.sh capture t1   # poll again once it's done
# -> you:
#    say OK
#
#    looprs:
#    I am sorry, but I cannot assist with this request. ...
.claude/skills/run-looprs/driver.sh stop t1
```

(Tested against `PROVIDER=local` — set it before `tui-launch` with
`env PROVIDER=local .claude/skills/run-looprs/driver.sh tui-launch t1`
if you need a deterministic provider; the driver itself doesn't set
one, so it inherits whatever `.looprs/provider.json` / `PROVIDER` your
shell already has.)

Verified provider-menu round-trip:

```
.claude/skills/run-looprs/driver.sh provider-launch p1
.claude/skills/run-looprs/driver.sh capture p1     # (settle, then capture again)
.claude/skills/run-looprs/driver.sh capture p1
# -> Select a provider
#    ┌───...
#    │anthropic
#    │openai
#    │local (Ollama)
.claude/skills/run-looprs/driver.sh send p1 Down
.claude/skills/run-looprs/driver.sh send p1 Enter
# picking any provider writes .looprs/provider.json and exits —
# the tmux session dies with it, no `stop` needed (but harmless if you do)
```

## Run (human path)

```
target/debug/looprs            # interactive REPL, real terminal
target/debug/looprs provider   # interactive menu, real terminal
target/debug/looprs tui        # alternate chat TUI, real terminal
target/debug/looprs -p "..."   # scriptable, one-shot — same as driver.sh prompt
```

## Test

```
cargo nextest run --workspace   # 433 tests as of this writing, 8 skipped (macOS-gated linux tests)
```

## Gotchas

- **First `capture` after any `*-launch` is often blank** — the pane
  hasn't drawn its first frame yet. Always `capture` once, discard,
  then `capture` again before trusting the output. No fixed sleep
  needed once you've done one throwaway capture — the tool-call
  round-trip is enough latency in practice.
- **`anthropic` provider fails in this environment** with `Anthropic
  API Error 400: Your credit balance is too low` — this is an account
  issue, not a code defect, but it means `prompt ... anthropic` and
  any anthropic-backed TUI turn will always show an error path here.
  Use `local` (real `ollama` call, verified working) for a response
  you can actually assert on.
- **The user's own message doesn't appear in the chat transcript
  until the turn completes.** Submitting shows "thinking..." /
  "waiting for response..." with nothing in the transcript pane (not
  even your own `you: ...` line) until the agent returns — only then
  does `static_transcript` get populated. Don't read a blank
  transcript during the busy state as "message wasn't sent."
- **Known bug (unfixed, see TODOs in source):** if a turn errors —
  auth failure, rate limit, network drop — `crates/looprs-tui/src/chat.rs`
  pushes the error into `live_text` and then unconditionally clears it
  before the next draw, so the TUI shows nothing: no reply, no error,
  just silence. Don't trust "no output" as "still working" — check
  `prompt`'s stderr directly (or `RUST_LOG`) if a `tui` turn seems to
  hang with the title back to idle.
- **Known bug (unfixed, see TODOs in source):** `looprs provider`'s
  local-model sub-menu can render a corrupted title (e.g. `Select a
  localdmodel` instead of `Select a local model`) because
  `crates/looprs-tui/src/lib.rs`'s `select()` creates a fresh
  `ratatui::Terminal` per call and its diff buffer doesn't know the
  physical terminal still has stale glyphs from the previous `select()`
  call in the same process. Cosmetic only — selection still works —
  but don't be surprised by garbled menu titles when chaining prompts.
- **Picking a provider in `looprs provider` exits the process**, which
  kills its single-pane tmux session. That's expected, not a driver
  bug — don't `stop` a session that already died on its own, and don't
  reuse a `provider-launch` session name without relaunching it first.
- **`send-keys -l` (literal) followed by a *separate* `send-keys`
  call for `Enter`** is the reliable pattern used above. Bundling
  multiple named keys into a single `send-keys` call (e.g. `Down Down
  Enter` as one invocation) was observed once to leak a stray literal
  character into the input box instead of registering as a keypress —
  issue one `send`/`type` per logical action and `capture` between
  ambiguous ones.
- **A `send t2 Enter` right after a `stop`+`tui-launch` (session name
  reused quickly) was seen twice to leak a stray literal `j` into the
  input box instead of submitting** — reproduced in the actual
  `looprs tui` session but *not* reproducible against a bare `cat`
  process under the same driver, so this looks specific to the app's
  crossterm `EventStream` racing the fresh terminal/raw-mode setup
  rather than a driver/tmux issue. Workaround: after `tui-launch`,
  always do the double-capture settle (see above) before the first
  `type`/`send`, and treat any stray character in the input box as a
  cue to `BSpace` it clean and retry rather than assuming Enter never
  arrived.
- **`Ctrl`-modified keys are not handled specially by the chat input.**
  `handle_input_key` in `crates/looprs-tui/src/chat.rs` matches only
  `KeyCode`, ignoring `KeyModifiers` — sending `C-u` (expecting
  "clear line") instead appends a literal `u` to the buffer. Only
  `Backspace` clears characters (one at a time); there's no
  clear-line shortcut.
- `.looprs/provider.json` is gitignored — driving `provider-launch`
  through to selection mutates your local provider config as a side
  effect. Harmless for verification, but re-run `provider-launch` and
  pick your usual provider afterward if you care which one is active.

## Troubleshooting

- `error connecting to /private/tmp/tmux-501/looprs_driver` on
  `stop`/`stop-all` — no sessions are up on that socket; harmless,
  ignore (the driver's own `kill-session`/`kill-server` calls already
  swallow this).
- `Anthropic API Error 400 ... credit balance is too low` from
  `prompt ... anthropic` — see Gotchas; switch to `local`.
- Blank `capture` output — see the "first capture is blank" Gotcha;
  capture again.
