# Commands

Repo-level custom commands loaded from `.looprs/commands/*.yaml`.

## Commands in this repo

The canonical, always-current list is `/help`'s own message text
(`.looprs/commands/help.yaml`) — reproduced here:

| Command | Aliases | Action | Notes |
|---------|---------|--------|-------|
| `/help` | `/h` | message | Lists available custom commands |
| `/model [provider[/model]]` | `/m` | switch_provider | Show or switch provider in-session, e.g. `/model openai/gpt-4o` |
| `/models` | `/ls-models` | list_models | Grouped discovery: current, providers, local, tiers, remote catalogs |
| `/check` | | shell | `cargo check --workspace` |
| `/build` | | shell | `cargo build --workspace --release` |
| `/test` | | shell | `cargo test --lib` |
| `/lint` | | shell | `cargo clippy --all-targets -- -D warnings` |
| `/fix` | | shell | `cargo clippy --fix --allow-dirty --allow-staged` |
| `/doc` | | shell | `cargo doc --workspace --no-deps --open` (not injected) |
| `/git` | `/log` | shell | `git log --oneline -10` |
| `/diff` | | shell | `git diff HEAD` |
| `/commit <message>` | | shell | `git add -A && git commit -m <message>` |
| `/refactor` | `/r` | prompt | Refactor prompt template sent to the LLM |
| `/reset-model <name>` | | shell | Runs `~/.looprs/scripts/reset-model.rs`, sets default model in `models.toml` |
| `/model-status` | | shell | Runs `~/.looprs/scripts/model-status.rs` — magi model version/training status |
| `/score-session [n]` | | shell | Runs `~/.looprs/scripts/score-session.rs` — scores last N interactions via OpenAI judge |
| `/fine-tune` | | shell | Runs `~/.looprs/scripts/fine-tune.rs` — flags session for RL training |
| `/outsource` | | outsource | Prints the outsource provider/model from `~/.looprs/models.toml` |

Plus built-ins (not YAML-defined): `/q`, `exit`, `quit` (exit session), `/c`, `clear` (clear conversation history).

## Format (YAML)

```yaml
name: command
description: Short description
aliases:
  - alias
action:
  type: message|prompt|shell|switch_provider|outsource
  # for message:
  text: "..."
  # for prompt:
  template: "..."
  variables: {}          # optional
  # for shell:
  command: "..."
  inject_output: true
  # switch_provider and outsource take no extra fields
```

`{args}` in a `shell` command's `command:` string is substituted with the
text typed after the command name.

## Usage

```
/help
/model openai/gpt-4o
/check
/test
/lint
/refactor
```
