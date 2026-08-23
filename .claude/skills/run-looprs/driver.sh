#!/bin/sh
# Driver for exercising the looprs CLI/TUI programmatically.
# Two paths:
#   1. Direct invocation (`prompt`) — scriptable mode, no terminal needed.
#      Covers agent/provider/hook logic: most PRs live here.
#   2. tmux driver (`*-launch`, `type`, `send`, `capture`, `stop`) — for
#      the interactive REPL, `looprs provider`, and `looprs tui` surfaces
#      added by crates/looprs-tui.
#
# Run from the repo root (<unit> = looprs workspace root).
#
# Usage:
#   .claude/skills/run-looprs/driver.sh build
#   .claude/skills/run-looprs/driver.sh prompt "<text>" [PROVIDER]
#   .claude/skills/run-looprs/driver.sh tui-launch <session>
#   .claude/skills/run-looprs/driver.sh provider-launch <session>
#   .claude/skills/run-looprs/driver.sh type <session> "<literal text>"
#   .claude/skills/run-looprs/driver.sh send <session> <tmux-key-name>
#   .claude/skills/run-looprs/driver.sh capture <session>
#   .claude/skills/run-looprs/driver.sh stop <session>

set -eu
TMUX_SOCKET=looprs_driver
BIN=target/debug/looprs

cmd="${1:-}"
shift || true

case "$cmd" in
  build)
    cargo build -p looprs-cli
    ;;

  prompt)
    text="${1:?usage: prompt <text> [PROVIDER]}"
    provider="${2:-anthropic}"
    PROVIDER="$provider" "$BIN" -p "$text"
    ;;

  tui-launch)
    session="${1:?usage: tui-launch <session>}"
    tmux -L "$TMUX_SOCKET" kill-session -t "$session" >/dev/null 2>&1 || true
    tmux -L "$TMUX_SOCKET" new-session -d -s "$session" -x 100 -y 30 "$BIN tui"
    ;;

  provider-launch)
    session="${1:?usage: provider-launch <session>}"
    tmux -L "$TMUX_SOCKET" kill-session -t "$session" >/dev/null 2>&1 || true
    tmux -L "$TMUX_SOCKET" new-session -d -s "$session" -x 100 -y 30 "$BIN provider"
    ;;

  type)
    session="${1:?usage: type <session> <text>}"
    text="${2:?usage: type <session> <text>}"
    tmux -L "$TMUX_SOCKET" send-keys -t "$session" -l "$text"
    ;;

  send)
    session="${1:?usage: send <session> <tmux-key-name>}"
    key="${2:?usage: send <session> <tmux-key-name>}"
    tmux -L "$TMUX_SOCKET" send-keys -t "$session" "$key"
    ;;

  capture)
    session="${1:?usage: capture <session>}"
    tmux -L "$TMUX_SOCKET" capture-pane -t "$session" -p
    ;;

  stop)
    session="${1:?usage: stop <session>}"
    tmux -L "$TMUX_SOCKET" kill-session -t "$session" >/dev/null 2>&1 || true
    ;;

  stop-all)
    tmux -L "$TMUX_SOCKET" kill-server >/dev/null 2>&1 || true
    ;;

  *)
    echo "unknown command: $cmd" >&2
    exit 1
    ;;
esac
