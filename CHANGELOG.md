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

### Changed
- Agent now uses provider abstraction (`dyn LLMProvider`) instead of hardcoded Anthropic logic
- CLI now displays provider name and model in header (e.g., "anthropic/claude-3-opus")
- Tool definitions now derive Debug for better error messages

### Technical
- Created `src/providers/` module with:
  - `mod.rs` - Trait definition and factory function
  - `anthropic.rs` - Anthropic provider implementation
  - `openai.rs` - OpenAI provider implementation  
  - `local.rs` - Ollama provider implementation
- Provider trait supports: infer(), name(), model(), validate_config(), supports_tool_use()
- Request/response types: `InferenceRequest`, `InferenceResponse`, `Usage`

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
