# Changelog

All notable changes to this project are documented in this file.

## [unreleased]

### Chores
- Anchor 13 tracked ideas as IDEA() comments across codebase
- Update handoff state + fmt residue in agent.rs
- Session close 2026-07-16
- Centralize secret loading via source_up

### Documentation
- Sync command actions and pre-push docs

### Features
- Implement Q1-Q5 and close L1 (hex Phase 1)
- Implement M1-M5 and L2-L3
- Enable build+lint checks and add unit tests
- Hex Phase 2 — inject ToolExecutor port into Agent
- McpToolExecutor — route agent tool calls to MCP server
- Implement run_turn_streaming and infer_stream port
- Add looprs-tui crate with provider menu and chat TUI
- Stream scriptable -p prompt output token-by-token
- Add BAML tool-calling and real token usage
- Add model catalog listing and bump workspace to 0.5.2
- Add /models command metadata and help docs
- Add OpenAI streaming support and backlog updates
- Extract shared macro_rules crate

### Fixes
- Port bash/python heredoc commands to rust-script and internal actions
- Remove redundant use dirs, collapse nested ifs, allow dead_code on mcp stubs
- Redundant closure, dead_code on mcp helpers
- Resolve op:// 1Password refs in API key env vars
- Correct xtask pre-push invocation in hook and docs
- Avoid unreleased looprs transcript APIs in published build
- Install protoc in all CI jobs
- Force stable toolchain for cargo-rail install
- Install taskit and cargo-rail with stable
- Use taskit commands in workflow
- Install llvm-cov and pin nightly toolchain
- Use nightly cargo-fuzz and satisfy clippy format lint
- Run fuzz on nightly and resolve clippy format lints
- Satisfy clippy uninlined format args
- Introspect corrections — report and index
- Address review findings for release and routing

### Refactoring
- Source session context from git and doob

### Tests
- Expand multi-dimensional test coverage across looprs\n## [0.5.1] - 2026-07-13

### Chores
- Remove demo noise, keep silent git context injection
- Add Rust devcontainer with CI tools
- Adopt cargo-rail for CI and local workflows
- Cargo-rail unify, release config, graph in CI
- Release v0.5.1

### Features
- Add /check /build /fix /doc /git /diff /commit commands; interpolate {args} in shell actions
- Gemini provider, statusline, /diff color, model fixes

### Fixes
- Remove redundant else branch in shorten_model\n## [0.5.0] - 2026-07-13

### CI
- Rewrite workflows to use taskit
- Install required tools and configure coverage crate

### Chores
- Add workspace package metadata (authors, repository, homepage, docs)
- Add version to path dependencies for crates.io publish
- Align xtask version with workspace; update health baseline to v0.4.0
- Release v0.5.0

### Documentation
- Add 0.4.1 entries for BAML backend, .env.nu, /model, install

### Features
- Add context compaction via sliding window in run_turn
- Implement ObservationManager persist/load_from via SQLite
- Implement mcp_tool_definitions via JSON-RPC tools/list
- Implement bundled_agents with reviewer, planner, debugger
- Implement ideas 1-5, stubs 6-10, miette diagnostics on all error types
- Add BAML inference backend + in-session provider switching
- Load .env by walking up from cwd at startup
- Load .env.nu via nushell instead of dotenvy

### Fixes
- Drop --output flag unsupported by published taskit version
- Allow dead_code on mcp_tool_definitions until wired into agent
- Derive Default for SessionStoreBackend
- Fix .env.nu parsing — emit KEY=VALUE lines instead of JSON

### Tests
- Verify Agent runs fully through ports with no real I/O\n## [0.4.0] - 2026-07-12

### Build
- Vendor openssl and bundle rusqlite for cross-compile

### CI
- Add 30s fuzz job on push to main

### Chores
- Add gitignore entries and track missing source files
- Gitignore fuzz corpus and artifacts
- Remove seed corpus from git tracking (now gitignored)
- Add taskit validation workflow
- Update Cargo.lock and handoff state
- Ignore fuzz/target/ build artifacts
- Add IDEAS.md and TODO/unimplemented! stubs for all 10 feature ideas
- Bump workspace version to 0.4.0

### Documentation
- Record session 4 — workspace restructure and v0.3.1
- Prune resolved HANDOFF items, add .worktrees to gitignore
- Add architecture report artifacts
- Remove stale external project references
- Update README and handoff state
- Document looprs repo config

### Features
- Implement scenario execution in mockstation (TODO#8)
- Add dynamic statusline prompt and per-turn context injection
- Add assert_inference_provider_contract to test_contracts

### Fixes
- Correct test fixture path and disable auto-bump workflow
- Use CommandFailed error for rg execution failure
- Replace deprecated env::home_dir with dirs
- Fix pre-existing clippy warnings
- Collapse outsource.yaml Python to single-line to fix YAML block scalar parse error
- Replace redundant if-let with is_some() in proptest
- Replace proptest stubs with ignored unit tests to satisfy clippy

### Other
- Raise rustqual score from 58.4% to 88.0%
- Add observability.rs tests
- Add models_config.rs tests
- Add ToolError tests
- Add fs_mode.rs tests
- Implement scenario execution (TODO#8)
- Revert "docs(depgraph): add architecture report artifacts"

This reverts commit b2db810a515310deb7d18f107f5061bc752028d0.
- Refactor/agent-architecture-pass into main
- Update state 2026-07-08 — dynamic statusline, YAML fix, config docs
- Docs/looprs-config-setup — CLAUDE.md, conformance contracts, repl tests, fuzz corpus+CI

### Refactoring
- Extract build_system_prompt from run_turn
- Extract log_inference from run_turn
- Inject SessionStore into Agent constructor
- Inject UserOutput into Agent constructor
- Remove dead BoolHelper trait
- Encapsulate Agent fields behind builder methods
- Split mockstation workspace

### Tests
- Add fuzz targets, property tests, and Kani proofs
- Add unit tests for observability.rs
- Add unit tests for models_config.rs
- Add unit tests for tools/error.rs
- Add unit tests for fs_mode.rs
- Add InferenceProvider conformance calls to all providers
- Add unit and proptest coverage for repl.rs
- Add seed corpus for fuzz_parse_skill and fuzz_strip_ansi\n## [0.3.1] - 2026-05-08

### Chores
- Bump to 0.3.1\n## [0.2.4] - 2026-05-08

### Chores
- Bump to 0.2.4\n## [0.2.3] - 2026-05-08

### Chores
- Restructure as proper virtual workspace
- Bump to 0.2.3\n## [0.2.2] - 2026-05-08

### Chores
- Bump to 0.2.2\n## [0.2.1] - 2026-05-08

### Chores
- Remove looprs-gui binary and bump to 0.2.1

### Features
- Add SqliteSessionStore adapter implementing SessionStore port
- Cannibalize model_badge and system_monitor from looprs-desktop
- Extract BAML domain types; remove desktop crates\n## [0.2.0] - 2026-05-07

### Build
- Add Makefile for common development operations

### CI
- Add semantic versioning workflow for automatic version bumping

### Chores
- Add cargo metadata
- Add license and toolchain
- Fix clippy format warnings
- Add local development files to gitignore
- Remove prek dependency from Makefile
- Ignore markdown files except README
- Add looprs-desktop crate to workspace
- Add planning-with-files skill to .claude
- Track mockstation scaffolding and update command docs
- Add proposed compaction & artifact patches (draft)
- Ignore local observability logs
- Clean up unused imports in context system
- Bump looprs, looprs-core, looprs-cli to 0.2.0

### Documentation
- Add structure modernization design
- Add structure modernization plan
- Add app description and setup instructions to README
- Update CHANGELOG with recent additions
- Add roadmap section with planned tool improvements
- Update README with rg/fd installation and performance notes
- Phase 7 - comprehensive provider documentation
- Rewrite README as unified abstraction layer
- Update roadmap with Phase 2 (hooks + jj + bd)
- Update roadmap to mark Phase 2c complete
- Add fine-tuning section for local models
- Update CHANGELOG with skills redesign
- Update CHANGELOG with skill loader implementation
- Consolidate CHANGELOG entries and fix duplicate sections
- Update README with versioning and skill loader status
- Update architecture documentation
- Add onboarding demo wizard design
- Add demo onboarding hook
- Add onboarding demo implementation plan
- Fix dry-run command example in scripts README
- Add pipeline design
- Add validated roadmap backlog and planning slices
- Update documentation and expose observability module
- Document desktop UI
- Add generative slot components design
- Add implementation plan for generative slot components
- Add local-first + magi integration design spec
- Add local-first + magi integration implementation plan
- Add models.toml.example and magi integration setup guide
- Audit and trim README
- Close HX-003/005/006/007 in handoff — hexagonal refactor complete

### Features
- Add extensibility framework with commands, skills, agents, rules, and hooks
- Add rg subprocess support for faster pattern matching
- Phase 1 - add provider abstraction layer
- Phase 2-3 - add OpenAI and Local providers
- Phase 5-6 - integrate providers into Agent and CLI
- Phase 8 - config file support for provider settings
- Jj and bd integration with SessionContext
- Event system wired into REPL and agent
- Session observations MVP - incremental learning system
- Dynamic max_tokens based on model and change default to gpt-5.2
- CLI argument parsing for scriptable mode (TDD)
- Fix OpenAI API parameter handling and add comprehensive documentation
- Implement hook context injection (Phase 2b)
- Complete Phase 2b - repo-level hooks, context injection, and approval gates
- Implement command parser for custom slash commands
- Implement file reference resolver with @filename syntax
- Implement skill loader with TDD
- Integrate skills into REPL with auto-trigger
- Implement rule evaluator for constraint checking
- Add onboarding demo flag
- Update onboarding flag safely
- Add console prompts
- Add onboarding action types
- Add local context conditions
- Execute onboarding actions
- Pass prompt callbacks
- Add machine-readable logging mode
- Add version bumping script and changelog organization
- Add delegated agent routing metadata and registry wiring
- Add internal skill discovery parity and loader integration
- Capture tool-use ids and append turn traces
- Add initial Freya GUI one-turn runner
- Add optional SDK-backed OpenAI and Anthropic providers
- Add fs_mode guardrails with REPL toggle
- Add initial looprs-desktop GUI application
- Add menu-based app flow and interactive mockstation
- Add structured observability infrastructure with SQLite persistence
- Enhance UI responsiveness and chat persistence integration
- Add BAML client crate
- Add BAML-backed generative UI
- Add websocket test scaffolding
- Implement comprehensive testing architecture
- Implement context-aware generative UI system
- Add generative components module structure
- Implement GenText primitive with builder pattern
- Integrate GenerativeContext with Freya context system
- Implement GenContainer primitive with styling
- Add demo screen showing generative slots
- Add SessionLogger with JSONL session event logging
- Add ModelsConfig for ~/.looprs/models.toml provider tier routing
- Wire SessionLogger into agent lifecycle events
- Add scorer module with OpenAI interaction scoring and pair extraction
- Auto-trigger OpenAI scoring on tool error and on-repeat (>=3 calls)
- Add /model-status /fine-tune /reset-model /score-session /outsource commands
- Add model badge to desktop UI (version, reward, training status)

### Fixes
- Resolve all clippy warnings - format args and dead code
- Prevent duplicate tag error in bump-version workflow
- Jail tool paths and fail-closed hooks
- Sanitize console output via ui boundary
- Write onboarding flag within target dir
- Keep message outputs in results
- Omit temperature for gpt-5 OpenAI models
- Cap tool result context size in history
- Run commands via system shell
- Avoid signal read/write loop in effects
- Patch freya-testing and add serial test execution
- Fix Freya component integration in context_demo
- Escape example template variables in sentiment_ui schema
- Align implementation with spec requirements
- Improve error handling and API consistency
- Resolve code quality issues in looprs-desktop
- Address code review findings from ralph-loop
- Session_log error handling, path return type, dir creation
- Models_config home dir error propagation and Clone derive
- Session_logger visibility and panic-free fallback
- Scorer db params and module ordering
- Scorer db write failure logs and continues instead of propagating
- OnRepeat trigger fires exactly at count==3, not repeatedly
- Command error handling, model name validation, safe TOML patching
- Badge refresh loop, limit reward to last 50, insta snapshot test
- Badge refresh loop, limit reward to last 50, insta snapshot test
- Model badge quality fixes (dead code, Path, BTreeMap, labels)
- Log SessionEnd, outsource command reads config

### Other
- Add pre-commit hooks with prek and refactor modules
- Add CI, bacon config, and rust-analyzer notes
- Bump version to 0.1.1 and update changelog
- Merge pull request #3 from 89jobrien/release/merge-structure-modernization

Merge structure-modernization branch
- Phase 2c: Hook file loading system

- Implemented hook YAML parsing and execution
  - Hook struct with trigger events, conditions, and action types
  - HookRegistry for loading from ~/.looprs/hooks/
  - HookExecutor for running actions on events
  - Action types: Command (shell), Message (console), Conditional (branching)
  - Condition types: on_branch:X (check branch), has_tool:X (check PATH)

- Wire into Agent and REPL
  - Agent loads hooks via with_hooks() builder method
  - execute_hooks_for_event() public method
  - Hook execution on all event types (SessionStart, PostToolUse, SessionEnd, etc.)
  - REPL loads hooks from ~/.looprs/hooks/ on startup

- Dependencies and infrastructure
  - Added serde_yaml (v0.9) for YAML parsing
  - Created src/hooks/{mod,parser,executor}.rs (1000+ lines)
  - Graceful degradation: missing hooks dir, bad YAML, failed commands all handled

- Documentation and examples
  - Updated README with hooks section (60 lines)
  - Updated CHANGELOG with Phase 2c details
  - Example hook: ~/.looprs/hooks/SessionStart.yaml

- Tests: All 54 tests passing (49 lib + 4 bin + 1 smoke)
- Apply rustfmt formatting
- Added .env to .gitignore
- Fix OpenAI message format compatibility

OpenAI's API uses a different format than Anthropic for tool calls:
- Assistant tool calls: use tool_calls array instead of tool_use content blocks
- Tool results: separate messages with role 'tool' instead of tool_result blocks

Changes:
- Rewrote message conversion in src/providers/openai.rs to use OpenAI format
- Updated documentation in .github/copilot-instructions.md

Fixes 'Invalid value: tool_use' error when using OpenAI models.
- Updated readme
- Load .env file in create_provider()
- Remove environment-dependent bd tests

- Remove test_bd_repo_detection and test_list_open_issues_no_bd_repo
- These tests assumed .beads directory doesn't exist, but looprs uses bd
- Keep parse_bd_issues tests that test actual parsing logic
- Update providers
- Align skills architecture with Anthropic Agent Skills standard

- Updated .looprs/skills/README.md to match Anthropic's SKILL.md format
  - Changed from JSON/progressive-learning to YAML/execution-focused
  - Documented bundled resources (scripts/, references/, assets/)
  - Added design principles: concise, progressive disclosure, degrees of freedom
- Created example skills demonstrating standard format:
  - rust-testing: Simple SKILL.md example
  - rust-error-handling: Full example with scripts and references
- Updated bd issue looprs-l4r with corrected requirements
- Updated main README.md roadmap to reflect new skills architecture

Key changes:
- Skills now follow industry standard (agentskills.io)
- Focus on operational guides vs educational tutorials
- YAML frontmatter (name + description) triggers skills
- Progressive disclosure: metadata → SKILL.md → resources
- Description field is primary trigger mechanism
- Add explicit triggers field to skills architecture

- Add 'triggers' as required field in YAML frontmatter
- Triggers are keywords/phrases for skill activation
- Case-insensitive substring matching with OR logic
- Supports both auto-triggering and explicit $skill-name invocation
- Updated skills README with trigger mechanism documentation
- Updated example skills with trigger lists:
  - rust-testing: test-related keywords
  - rust-error-handling: error handling keywords
- Updated bd issue looprs-l4r with trigger requirements

Benefits:
- More deterministic than description-based inference
- Clear, explicit activation mechanism
- User-friendly: natural language triggers
- Allows fine-grained control over skill activation
- Add ToolArgs helper and remove ApiConfig
- Add ToolArgs helper and remove ApiConfig
- Add newtypes for model/tool identifiers
- Add structured provider and agent errors
- Fix clippy lints: add Default for SkillRegistry and use inlined format args
- Add kan sync design

[version-bump: 0.1.9 → 0.1.10]
- Add external CLI adapter layer
- Run jj/bd/kan and bd observations via plugins
- Use plugins for rg/fd availability and rg exec
- Use PATH resolver for has_tool checks
- Add NamedTool interface and binary adapters
- Route jj/bd/kan through binary adapters
- Route bd and rg probes through binary adapters
- Update root README architecture and commands

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Update repo .looprs overview

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Document repo commands

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Document repo hooks

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Document repo skills

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Document repo rules

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Document repo agents

Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-opencode)

Co-authored-by: Sisyphus <clio-agent@sisyphuslabs.ai>
- Refactor CLI architecture: split REPL into separate module and add app config

- Extract REPL functionality from main.rs into dedicated repl.rs module
- Add app_config.rs for centralized application settings management
- Update imports and module structure across providers, tools, and core components
- Improve code organization and maintainability
- Change copyright from 89jobrien to Joseph O'Brien

Updated copyright holder name in LICENSE file.
- Merge pull request #4 from 89jobrien/89jobrien-patch-1

Change copyright from 89jobrien to Joseph O'Brien
- Going open
- Fix OpenAI reasoning model temperature parameter error

- Set default temperature to 0.2 (more deterministic outputs)
- Add model capability detection for temperature support
- Reasoning models (o1, o3) don't support temperature parameter
- Only send temperature to API when model supports it
- Reasoning models use max_completion_tokens parameter
- Add comprehensive tests for model detection

Fixes: 'Unsupported value: temperature does not support 0.699...071' error
- Add new hook actions for confirm, prompt, secret_prompt, set_env, and set_config

Extend hook action types with user confirmation prompts, secret input,
environment variable setting, and configuration overrides to improve
hook flexibility and automation capabilities.
- Fix duplicate hook parser test
- Fix clippy warnings
- Config ownership and seed command

- Stop writing .looprs/config.json: user owns it
- Add .looprs/state.json for app state (onboarding.demo_seen)
- AppConfig::load() overlays onboarding from state file
- Hook set_config writes to state, not config
- save_configs() is a no-op; :set/:unset session-only
- Add 'looprs seed [DIR]' to write config.json.example and provider.json.example
- Spec: specs/config-ownership-and-seed-command.md

Co-authored-by: Cursor <cursoragent@cursor.com>
- Update README.md with clearer looprs description

Revised the description of looprs to clarify its purpose and functionality.
- Merge pull request #5 from 89jobrien/89jobrien-patch-1

Update README.md with clearer looprs description
- Initial plan
- Update .gitignore
- Update README.md
- Merge branch 'main' of github.com:89jobrien/looprs
- Merge branch 'main' into copilot/script-version-bumping
- Merge pull request #6 from 89jobrien/copilot/script-version-bumping

Add automated version bumping with changelog generation
- Began implementing the agentic pipeline
- Standardize import ordering with rustfmt
- Delete third_party directory
- Merge pull request #7 from 89jobrien/89jobrien-patch-1

Delete third_party directory

### Performance
- Stream read output

### Refactoring
- Add lib crate and bin cli
- Split tools into submodules
- Extract bootstrap logic to runtime module
- Enhance path sanitization and glob pattern validation
- Tidy mock services
- Harden context compaction
- Simplify startup and lazy-init mockstation
- Add Default impl for GenText
- Extract hexagonal ports and adapters to looprs-core (#12)
- Unify LLMProvider with InferenceProvider port from looprs-core (#13)
- Wire Agent through UserOutput port, add UiOutput and NullOutput adapters (#14)

### Tests
- Add cli smoke test
- Make cli smoke env-safe
- Serialize cli smoke env changes
- Add comprehensive test coverage for api.rs and agent.rs
- Version bump hook
- Avoid cwd mutation in onboarding flag test
- Cover onboarding parent dir creation
- Validate new action fields
- Cover env and config conditions
- Cover onboarding action execution
- Use manifest dir for demo hook
- Add env-gated live LLM smoke test\n
