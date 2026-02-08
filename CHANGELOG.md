# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Multi-provider LLM support**: Switch between Anthropic, OpenAI, and Local Ollama without code changes
  - Anthropic provider with Claude 3 models
  - OpenAI provider with GPT-4/GPT-5 models
  - Local provider for Ollama (open-source models)
- **Provider detection**: Auto-detect provider from environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or Ollama availability)
- **Flexible configuration**: 
  - `PROVIDER` env var to force specific provider
  - `MODEL` env var to select model per provider
  - `.env.example` with complete setup instructions
- **Provider selection docs**: Added comprehensive provider configuration guide to README
- **Graceful degradation**: Local models work without tool use support
- `async-trait` dependency for provider trait pattern
- **jj integration**: Auto-detect jujutsu repos and read status, branch, recent commits
  - `jj::get_status()` - Current branch, commit, description
  - `jj::get_recent_commits()` - Last N commits from main branch
  - Graceful fallback if not in jj repo or jj not installed
- **bd integration**: Auto-detect beads.db and list open issues
  - `bd::list_open_issues()` - Query open issues with title, status, priority
  - Parse newline-delimited JSON from bd command
  - Graceful fallback if not in bd repo or bd not installed
- **SessionContext collection**: Automatically gather repo state at startup
  - `SessionContext::collect()` - Gathers jj status, recent commits, open issues
  - `format_for_prompt()` - Human-readable formatted context for injection
  - Display context on session start (when available)
- **Event system**: Session lifecycle and execution events
  - `Event` enum with 8 event types (SessionStart, SessionEnd, UserPromptSubmit, PreToolUse, PostToolUse, InferenceComplete, OnError, OnWarning)
  - `EventContext` for passing data through events (session context, user message, tool name, error info)
  - `EventManager` for registering and firing event handlers
  - Events fire at key points in REPL cycle and agent execution
  - Ready for hook file loading in Phase 2
- **Session observations** (Incremental Learning System)
  - Auto-capture all tool executions (bash, read, write, grep, glob, edit)
  - `Observation` struct with tool_name, input, output, timestamp, session_id
  - `ObservationManager` for capturing and persisting observations
  - On PostToolUse: automatically capture tool usage
  - On SessionEnd: save observations to bd as issues (tag: observation)
  - On SessionStart: load recent observations and display in REPL
  - Graceful degradation if bd not available
  - Foundation for Claude-mem style learning across sessions
- **Hook file loading** (Event-driven automation)
   - `Hook` struct with trigger event, condition, and action list
   - `HookRegistry` for loading YAML hooks from `~/.looprs/hooks/`
   - YAML parsing with serde_yaml (graceful error handling)
   - `HookExecutor` for running hook actions on events
   - Action types: Command (shell execution), Message (console output), Conditional (branching)
   - Condition types: `on_branch:X` (check git/jj branch), `has_tool:X` (check PATH)
   - Hook execution on all event types (SessionStart, PostToolUse, SessionEnd, etc.)
   - Graceful degradation: missing hooks dir, bad YAML, failed commands all handled silently
   - Example hook: `~/.looprs/hooks/SessionStart.yaml` with message action

### Changed
- Agent now uses provider abstraction (`dyn LLMProvider`) instead of hardcoded Anthropic logic
- Main REPL loads hooks from `~/.looprs/hooks/` on startup
- Main REPL executes hooks on SessionStart/SessionEnd
- Agent now has public HookRegistry and execute_hooks_for_event() method
- Agent executes hooks on all event types (UserPromptSubmit, PostToolUse, etc.)
- CLI now displays provider name and model in header (e.g., "anthropic/claude-3-opus")
- Tool definitions now derive Debug for better error messages
- Main REPL now collects and displays SessionContext at startup
- Main REPL fires SessionStart event on init, SessionEnd on exit
- Main REPL displays recent observations before prompt
- Main REPL saves observations to bd on exit
- Agent fires UserPromptSubmit, InferenceComplete, PreToolUse, PostToolUse, OnError events
- Agent now has public EventManager for registering handlers
- Agent now has public ObservationManager for accessing captured observations

### Technical
- Created `src/providers/` module with:
  - `mod.rs` - Trait definition and factory function
  - `anthropic.rs` - Anthropic provider implementation
  - `openai.rs` - OpenAI provider implementation  
  - `local.rs` - Ollama provider implementation
- Created `src/jj.rs` module with jujutsu integration:
  - `JjStatus` struct with branch, commit, description
  - Repo detection via `.jj` directory
  - Command execution via subprocess
- Created `src/bd.rs` module with beads.db integration:
  - `BdIssue` struct with id, title, status, priority
  - Issue listing via bd command
  - JSON parsing for issue data
- Created `src/context.rs` module for context collection:
  - `SessionContext` struct aggregating jj + bd + kanban data
  - Prompt formatting for context injection
  - Optional fields for graceful degradation
- Created `src/events.rs` module for event system:
  - `Event` enum (8 variants for lifecycle + execution events)
  - `EventContext` struct with builder pattern
  - `EventManager` with HashMap-based handler registry
- Created `src/observation.rs` module for session observations:
  - `Observation` struct with Unix timestamp-based IDs
  - Serialization to bd issue format
  - Optional context/summary field
- Created `src/observation_manager.rs` module for observation management:
  - `ObservationManager` struct for capturing and persisting
  - Auto-save to bd with proper tagging
  - `load_recent_observations()` for SessionStart injection
  - `EventContext` struct with builder pattern (session_context, user_message, tool_name, tool_output, error, warning, metadata)
  - `EventManager` with HashMap-based handler registry
  - Full test coverage for event firing and multiple handlers

- Created `src/hooks/mod.rs` - Hook types and registry
   - `Hook` struct with name, trigger, condition, actions
   - `Action` enum (Command, Message, Conditional)
   - `HookRegistry` with load_from_directory() and hooks_for_event()
   - HashMap-based indexing by event type
- Created `src/hooks/parser.rs` - YAML parsing
   - `parse_hook()` reads and deserializes YAML hook files
   - Serde integration with `#[derive(Serialize, Deserialize)]`
   - Error handling for invalid YAML (returns Err, logged to stderr)
- Created `src/hooks/executor.rs` - Hook execution
   - `HookExecutor` with execute_hook() method
   - `HookResult` struct with output and injection key
   - `run_command()` for shell execution via `sh -c`
   - `eval_condition()` for simple condition evaluation (on_branch, has_tool)
   - `check_tool_available()` using `which` command
- Added `serde_yaml` dependency (v0.9) to Cargo.toml
- Updated `src/lib.rs` to export hooks modules and types
- Updated `src/agent.rs` to load hooks and execute on events
- Updated `src/bin/looprs/main.rs` to load hooks from home directory on startup
## [0.1.1] - 2026-02-07

### Added
- Makefile with common development operations (`make build`, `make test`, `make lint`, `make install`, etc.).
- Comprehensive README with app description, installation instructions, and setup guide.
- ripgrep (rg) subprocess support for grep tool (10-100x faster)
- fd detection for glob tool optimization
- Tool availability detection system

### Changed
- Refactored `src/main.rs` into focused modules.
- Updated dependencies and configuration for the new module layout.

## [0.1.0] - 2026-02-07

### Added
- Initial project setup.
