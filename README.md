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

## Fine-Tuning Local Models for Tool Calling

Local models (via Ollama) have limited tool-calling capabilities out of the box. However, you can fine-tune models to understand looprs' tool format using LoRA adapters.

### Why Fine-Tune?

- **Enable tool use** - Local models can learn to emit `[TOOL_USE ...]` markers
- **No API costs** - Run agents completely locally with tool support
- **Privacy** - Keep data on your machine while still getting tool capabilities
- **Fast inference** - Small fine-tuned models (300MB-7B) run quickly on consumer hardware

### Quick Start

**Option 1: Prompt Engineering (5 minutes, no GPU)**

Create a Modelfile with few-shot examples:

```
FROM functiongemma:latest

SYSTEM """Execute tool calls immediately.
Format: [TOOL_USE id=tool_NUM name=NAME]
{JSON}"""

MESSAGE user Show me README.md
MESSAGE assistant [TOOL_USE id=tool_1 name=read]\n{"path": "README.md"}

MESSAGE user List all Python files  
MESSAGE assistant [TOOL_USE id=tool_2 name=glob]\n{"pat": "**/*.py"}

PARAMETER temperature 0.1
```

```bash
ollama create looprs-fg -f Modelfile
MODEL=looprs-fg looprs -p "Read Cargo.toml"
```

**Limitations:** Works in isolation but may fail in full looprs context. Best for testing.

**Option 2: LoRA Fine-Tuning (15 minutes with GPU)**

Complete training scripts available in session workspace. Requires GPU (Google Colab T4 works great):

```bash
# Generate training data (23 examples covering all 6 tools)
python3 training-data-generator.py

# Train LoRA adapter (requires: torch, transformers, peft)
python3 train-lora.py  # ~15 min on T4 GPU

# Convert adapter to GGUF and import to Ollama
# (see README-FINETUNE.md for conversion steps)

# Use the fine-tuned model
MODEL=looprs-functiongemma looprs
```

**Benefits:** Proper weight updates, consistent behavior, handles edge cases better.

### Training Data Format

looprs uses text-based tool markers:

```
User: Show me README.md
Assistant: [TOOL_USE id=tool_123 name=read]
{"path": "README.md"}
User: [TOOL_RESULT id=tool_123]
# looprs
A unified abstraction layer...
Assistant: Here's the README content.
```

The training data generator creates examples for all built-in tools: `read`, `write`, `edit`, `glob`, `grep`, `bash`.

### Fine-Tuning Resources

Complete fine-tuning package with:
- `training-data-generator.py` - Generates synthetic examples
- `train-lora.py` - Full LoRA training with PEFT
- `README-FINETUNE.md` - Detailed guide with troubleshooting
- `Modelfile-v4` - Best prompt engineering approach
- `PROMPT-ENGINEERING-RESULTS.md` - Test results and findings

Files available in: `~/.local/state/.copilot/session-state/<session-id>/files/`

### Recommended Models

**For prompt engineering:**
- `functiongemma:latest` (300 MB) - Google's function-calling model
- `gemma:7b` (7B) - Larger, better instruction following

**For LoRA training:**
- Base: `google/gemma-2b-it` or `google/gemma-7b-it`
- Hardware: 16GB+ VRAM recommended, 8GB works with QLoRA
- Training time: ~15 minutes (T4), ~5 minutes (4090/A100)

### Integration

After fine-tuning, enable tool support in looprs:

```rust
// src/providers/local.rs
fn supports_tool_use(&self) -> bool {
    self.model.contains("looprs-functiongemma")
}
```

See `PROMPT-ENGINEERING-RESULTS.md` for detailed findings and next steps.

## Extensibility Framework

The `.looprs/` directory defines your agent configuration (provider, rules, skills, etc.).

## Architecture

- `src/bin/looprs/` - CLI entrypoint, argument parsing, interactive REPL
- `src/agent.rs` - Core orchestrator (messages, tools, events, hooks, observations)
- `src/providers/` - LLM backends (Anthropic, OpenAI, local)
- `src/tools/` - Built-in tools (read/write/edit/glob/grep/bash)
- `src/events.rs` + `src/hooks/` - Event system and hook execution
- `src/commands.rs` + `.looprs/commands/` - Command registry and repo command definitions
- `src/skills/` + `.looprs/skills/` - Skill loading and repo examples
- `src/context.rs` - SessionContext (jj/bd/kan snapshots at startup)

### Custom Commands

Define slash commands to execute common workflows. Commands are loaded from both user and repo directories with **repo precedence**.

**Example: `.looprs/commands/refactor.yaml`**
```yaml
name: refactor
description: Refactor code for readability
aliases:
  - r
action:
  type: prompt
  template: "Refactor this code for better readability..."
```

**Example: `.looprs/commands/test.yaml`**
```yaml
name: test
description: Run tests
aliases:
  - t
 action:
   type: shell
   command: cargo test --lib
   inject_output: true  # Add output to conversation context
```

**Usage:**
```
❯ /refactor
# Sends prompt template to LLM

❯ /test
# Runs cargo test, shows output, injects into context if inject_output: true
```

### Repo Commands (this repo)

Loaded from `.looprs/commands/`:

- `/help` (`/h`) - Show available custom commands
- `/refactor` (`/r`) - Prompt-only refactor request
- `/test` (`/t`) - `cargo test --lib` with output injected
- `/lint` (`/l`) - `cargo clippy --all-targets -- -D warnings` with output injected

**Action types:**
- `prompt` - Send template as message to LLM
- `shell` - Execute shell command, optionally inject output into context
- `message` - Display text to console

### File References

Reference files in your prompts using `@filename` syntax. The file contents will be automatically injected into the conversation.

**Usage:**
```
❯ Refactor @src/main.rs for better error handling
# File contents are injected with syntax highlighting context

❯ Compare @file1.rs and @file2.rs
# Multiple files can be referenced in one message

❯ /refactor @src/utils.rs
# Works in custom commands too
```

**Features:**
- Automatic path resolution from current working directory
- Security: blocks path traversal attempts (`../../../etc/passwd`)
- Supports subdirectories: `@src/modules/parser.rs`
- Graceful degradation: missing files show warning but don't break session

**Example output:**
```
Check @test.rs please

→ Resolved to:

Check 
```
// File: test.rs
fn test_example() {
    assert_eq!(1 + 1, 2);
}
```
 please
```

### Hook Loading

Hooks are loaded from two locations with **repo precedence**:
- **User hooks**: `~/.looprs/hooks/` (global, shared across all projects)
- **Repo hooks**: `.looprs/hooks/` (project-specific, checked into version control)

When both define a hook with the same name for the same event, **repo hooks override user hooks**.

```
.looprs/
├── provider.json          # Provider settings
├── config.json            # Global config
├── commands/              # Custom commands (/)
├── hooks/                 # Repo-level hooks (override user hooks)
├── skills/                # Skills with progressive disclosure ($)
├── agents/                # Agent role definitions (YAML)
└── rules/                 # Constraints and guidelines (Markdown)
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
    command: "jj log --no-pager -r 'main..' -n 5"
    inject_as: recent_commits
  - type: command
    command: "bd list --open --json"
    inject_as: open_issues
  - type: command
    command: "kan status --json"
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
- `command` - Execute shell command, optionally inject output into context with `inject_as`
  - Injected values are added to the LLM system prompt under "Additional Context from Hooks"
  - Large values (>2000 chars) are automatically truncated to prevent prompt bloat
  - **Approval gates**: Add `requires_approval: true` to prompt user before execution
  - Custom prompt: Use `approval_prompt: "Your message"` for user-friendly approval text
- `message` - Print message to console
- `conditional` - Run sub-actions if condition passes

**Approval gates example:**
```yaml
name: sensitive_operation
trigger: SessionStart
actions:
  - type: command
    command: "git push origin main"
    requires_approval: true
    approval_prompt: "Push changes to remote repository?"
```

User will see: `🔒 Approval required: Push changes to remote repository? [y/N]`

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
- [x] Provider configuration (env vars + config file)
- [x] jj (jujutsu) integration - repo state + recent commits
- [x] bd (beads.db) integration - open issues
- [x] SessionContext collection - auto-detect on startup
- [x] **Event system** (SessionStart, SessionEnd, PreToolUse, PostToolUse, OnError, OnWarning)
- [x] **Session observations** - Auto-capture tool use, store in bd
- [x] **Hook file loading** - Parse YAML from `~/.looprs/hooks/`
- [x] **Hook execution** - Fire hooks on events, execute shell commands
- [x] **Repo-level `.looprs/hooks/` support** - Load hooks from repo with precedence
- [x] **Context injection** - Inject hook outputs into LLM prompts via `inject_as` field
- [x] **Approval gates** - User approval for automated actions
- [x] **Command parser** - Custom slash commands from `.looprs/commands/`
- [x] **File reference resolver** - `@filename` syntax automatically injects file contents

### Phase 3: Extensibility Parsers (In Progress)
- [x] **Skill loader** - Load skills following Anthropic Agent Skills standard ✅
  - YAML frontmatter with name, description, triggers
  - `$skill-name` syntax for explicit invocation
  - Auto-triggering via keyword matching
  - Bundled resources (scripts/, references/, assets/)
  - Progressive disclosure (metadata → body → resources)
  - Dual-source loading (user + repo directories with precedence)
- [ ] **Agent dispatcher** - YAML-based role switching
- [ ] **Rule evaluator** - Constraint checking from markdown rules

### Phase 4: Advanced Features
- [ ] Session persistence (conversation history)
- [ ] Multi-turn context management
- [ ] Streaming response support
- [ ] Tool result caching
- [ ] Performance profiling
- [ ] Plugin system for custom tools
- [ ] Hook output storage (debugging)

## Dev

```bash
make build      # build release binary
make test       # run tests
make lint       # run clippy
make install    # install locally
```

Uses `prek` for pre-commit hooks (cargo test + clippy). See `Makefile` for all targets.

### Versioning

Patch versions increment automatically on every push via pre-push git hook:
- Bumps version in `Cargo.toml`
- Moves `[Unreleased]` content to new version section in `CHANGELOG.md`
- Amends commit with version bump changes
- Adds marker to prevent recursive bumping

Example: Push commit "feat: add feature" → automatically becomes version 0.1.4.

## License

MIT
