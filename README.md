# looprs

A unified abstraction layer for agentic AI. Looprs gives you a REPL that talks to LLMs (Claude, GPT, local models) and provides consistent interfaces for extending it with **commands** (`/`), **skills** (`$`), **agents**, **rules**, **hooks**, and **file references** (`@`).

Think of it as: LLM + tools + extensibility framework = everything you need to build agents.

## Install

```bash
git clone https://github.com/89jobrien/looprs.git
cd looprs
cargo build --release
./target/release/looprs
# or: cargo install --path .
```

## Configure

Pick an LLM provider:

```bash
# Anthropic (recommended, fastest setup)
export ANTHROPIC_API_KEY="sk-ant-..."
looprs

# OpenAI (GPT-4/GPT-5)
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4-turbo"
looprs

# Local (Ollama)
ollama serve  # in another terminal
export PROVIDER="local"
looprs
```

Or use `.looprs/provider.json` for persistent config. See `.env.example` for all options.

## Built-In Tools

**File operations:**
- `/read` - read files with line pagination
- `/write` - create/overwrite files
- `/edit` - replace text in files
- `/glob` - find files by name pattern (10-100x faster with `fd` installed)
- `/grep` - search file contents (10-100x faster with `rg` installed)
- `/bash` - execute shell commands

**Optional performance upgrades:**
```bash
cargo install ripgrep      # grep speedup
cargo install fd-find      # glob speedup
```

Both are detected automatically. Falls back to pure Rust if not installed.

## Extensibility Framework

The `.looprs/` directory defines your agent configuration (provider, rules, skills, etc.).

> Note: **Hooks are currently loaded from `~/.looprs/hooks/` (user-level)**. Repo-level hooks in `.looprs/hooks/` are **planned** but not yet implemented.

```
.looprs/
├── provider.json          # Provider settings
├── config.json            # Global config
├── commands/              # Custom commands (/)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
└── rules/                 # Constraints and guidelines (Markdown)
# (planned) hooks/         # Repo-level hooks (see Roadmap)
```

### SessionStart Context

When you start looprs, it automatically collects:

```
# Repository Status (jj)
- Branch: main
- Commit: abc123
- Description: Implement feature X

# Recent Commits (from jj)
- Fix: edge case in parser
- Feat: add new command syntax
- Docs: update README

# Open Issues (from bd)
- [#42] Parser refactor: high priority
- [#51] Add tests for X: normal priority
```

Example hook that injects context (user-level):

```yaml
# ~/.looprs/hooks/SessionStart.yaml
name: inject_context
trigger: SessionStart
actions:
  - type: command
    command: "jj log --no-pager -r 'main::' | head -5"
    inject_as: recent_commits
  - type: command
    command: "bd list --open"
    inject_as: open_issues
  - type: command
    command: "kanban_board --json"
    inject_as: board_state
```

Example command:

```
/code:refactor
  Description: Ask AI to refactor selected code
  Template: Refactor this code for readability: {selection}
```

The framework is ready to extend. Define hooks, commands, skills - all **without changing looprs core**.

### Event System

Looprs fires events throughout the session lifecycle for hooks to listen to:

```
SessionStart        → Session initialized, context available
UserPromptSubmit    → User message received, before processing
InferenceComplete   → LLM response complete
PreToolUse          → Tool about to execute (approval gate)
PostToolUse         → Tool executed successfully
OnError             → Error occurred
OnWarning           → Warning issued
SessionEnd          → Session closing
```

Register event handlers in your code:

```rust
agent.events.on(Event::SessionStart, |event, ctx| {
    println!("Session started with context: {:?}", ctx.session_context);
});

agent.events.on(Event::PreToolUse, |event, ctx| {
    println!("About to execute tool: {}", ctx.tool_name.as_ref().unwrap_or(&"unknown".to_string()));
});
```

### Hooks (Event-Driven Actions)

Define YAML hooks to run shell commands or inject context on events. Hooks live in `~/.looprs/hooks/` and execute automatically.

**Example hook file: `~/.looprs/hooks/SessionStart.yaml`**

```yaml
name: show_status
trigger: SessionStart
condition: has_tool:jj  # optional: only run if tool exists
actions:
  - type: command
    command: "jj log -r 'main::' | head -3"
    inject_as: recent_commits  # inject output into EventContext
  
  - type: message
    text: "Session started with context injected"

  - type: conditional
    condition: on_branch:main
    then:
      - type: message
        text: "You're on main branch"
```

**Event hooks:**
- `~/.looprs/hooks/SessionStart.yaml` - runs on session init
- `~/.looprs/hooks/PostToolUse.yaml` - runs after each tool execution
- `~/.looprs/hooks/SessionEnd.yaml` - runs on session exit
- etc. for all 8 event types

**Action types:**
- `command` - Execute shell command, optionally inject output into context
- `message` - Print message to console
- `conditional` - Run sub-actions if condition passes

**Conditions:**
- `on_branch:main` - Only execute if on specified branch (currently accepts "main" or "*")
- `has_tool:jj` - Only execute if tool is available in PATH

**Graceful degradation:**
- If `~/.looprs/hooks/` doesn't exist → no hooks run (works fine)
- If hook execution fails → warning printed, session continues
- If tool isn't available → condition fails silently, hook skipped

### Session Observations (Incremental Learning)

Looprs automatically captures what you do in sessions and stores observations for future reference:

```
User runs: cargo test
  ↓
Tool execution captured: bash cargo test → output
  ↓
Session ends (Ctrl-C)
  ↓
Observation saved to bd: "Observation: cargo test"
  ↓
Next session starts
  ↓
Recent observations displayed: "Observation: cargo test"
  ↓
AI can now reference past patterns
```

**How it works:**
- Every tool execution (bash, read, grep, etc.) is automatically captured
- On SessionEnd, observations are saved to bd as issues (tag: observation)
- On SessionStart, recent observations are loaded and displayed
- The AI can then reference "what we did last session" for continuity

**Example session output:**
```
>> looprs | anthropic/claude-3-sonnet | /home/dev/looprs

Repository Status
- Branch: main
- Commit: 119b0ba

Recent observations:
  1. Observation: cargo test - test result ok
  2. Observation: Fixed parser edge case
  3. Observation: Updated README
```

## Multi-Provider LLM

Looprs works with any major LLM:

| Provider | Setup | Models | Cost |
|----------|-------|--------|------|
| Anthropic | `ANTHROPIC_API_KEY` | Claude 3 (Opus/Sonnet/Haiku) | $$ |
| OpenAI | `OPENAI_API_KEY` | GPT-4/GPT-5 | $$$ |
| Local | `ollama serve` | llama2, mistral, neural-chat, etc. | Free |

**Auto-detects** from env vars. Force with `PROVIDER=anthropic \| openai \| local`.

Per-provider config: `.looprs/provider.json` or `MODEL=` env var.

## Roadmap

### Done ✅
- [x] Multi-provider LLM support (Anthropic, OpenAI, Local)
- [x] Fast search: grep + ripgrep, glob + fd
- [x] Extensibility framework (commands, skills, agents, rules, hooks, file refs)
- [x] Provider configuration (env vars + config file)
- [x] jj (jujutsu) integration - repo state + recent commits
- [x] bd (beads.db) integration - open issues
- [x] SessionContext collection - auto-detect on startup
- [x] **Event system** (SessionStart, SessionEnd, PreToolUse, PostToolUse, OnError, OnWarning)
- [x] **Session observations** - Auto-capture tool use, store in bd
- [x] **Hook file loading** - Parse YAML from `~/.looprs/hooks/`
- [x] **Hook execution** - Fire hooks on events, execute shell commands

### Phase 2b: Context Injection (Next)
- [ ] **Repo-level `.looprs/hooks/` support** - Load hooks from the current repo (in addition to `~/.looprs/hooks/`), with documented precedence/merge behavior
- [ ] **Context injection** - Inject hook outputs into LLM prompts
- [ ] **Approval gates** - User approval for automated actions
- [ ] **Hook output storage** - Persist hook results for debugging

### Phase 3: Extensibility Parsers
- [ ] Command parser for `/` prefix
- [ ] Skill loader with level tracking
- [ ] Agent dispatcher (YAML-based roles)
- [ ] Rule evaluator (constraint checking)
- [ ] File reference resolver (`@` prefix)

### Phase 4: Advanced Features
- [ ] Session persistence (conversation history)
- [ ] Plugin system (custom tools/commands)
- [ ] Performance profiling
- [ ] Concurrent hook execution

## Dev

```bash
make build      # build release binary
make test       # run tests
make lint       # run clippy
make install    # install locally
```

Uses `prek` for pre-commit hooks (cargo test + clippy). See `Makefile` for all targets.

## License

MIT
