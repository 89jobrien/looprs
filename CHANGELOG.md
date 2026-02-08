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

### Changed
- Agent now uses provider abstraction (`dyn LLMProvider`) instead of hardcoded Anthropic logic
- CLI now displays provider name and model in header (e.g., "anthropic/claude-3-opus")
- Tool definitions now derive Debug for better error messages
- Main REPL now collects and displays SessionContext at startup

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
