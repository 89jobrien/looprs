# looprs

A concise coding assistant REPL powered by language models. Looprs provides an interactive command-line interface for quick coding tasks, refactoring, and development assistance.

## Getting Started

### Prerequisites

- Rust 1.88 or later
- An API key for at least one LLM provider:
  - **Anthropic**: Set `ANTHROPIC_API_KEY` (recommended for fastest setup)
  - **OpenAI**: Set `OPENAI_API_KEY` (for GPT-4/GPT-5 models)
  - **Local Ollama**: Run `ollama serve` on localhost:11434

### Optional: Performance Tools

For faster searching and file discovery, install these modern tools:

```bash
# ripgrep - 10-100x faster than standard grep
cargo install ripgrep

# fd - Fast alternative to find
cargo install fd-find
```

These tools are **optional** - looprs works without them using pure Rust implementations. When installed, the grep and glob tools automatically use them for dramatically faster performance.

### Installation

Clone the repository and build the project:

```bash
git clone https://github.com/89jobrien/looprs.git
cd looprs
cargo build --release
```

The binary will be available at `target/release/looprs`.

### Running

Set your API key and run:

```bash
# Using Anthropic (recommended)
export ANTHROPIC_API_KEY="sk-ant-..."
looprs

# Using OpenAI
export OPENAI_API_KEY="sk-..."
looprs

# Using local Ollama
ollama serve  # in another terminal
looprs  # will auto-detect Ollama
```

You can also specify a model explicitly:

```bash
# Use specific Anthropic model
export MODEL="claude-3-sonnet-20240229"
looprs

# Use specific OpenAI model
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4-turbo"
looprs

# Force provider selection
export PROVIDER="local"  # or "anthropic", "openai"
looprs
```

Or install globally:

```bash
cargo install --path .
looprs
```

## Development

### Rust-analyzer

Recommended for editor diagnostics and code navigation:

```bash
rustup component add rust-analyzer
```

### Bacon

`bacon` provides fast local feedback loops using the jobs in `bacon.toml`.
CI uses `cargo test` and `cargo clippy` directly; bacon is for local dev only.

Install:

```bash
cargo install --locked bacon
```

Common jobs:

```bash
bacon check
bacon clippy
bacon test
```

### Pre-commit hooks

This repo uses `prek` (Rust-native, drop-in compatible with pre-commit) to run Rust checks on every commit.

Install once (pick one):

```bash
cargo install --locked prek
```

```bash
pipx install prek
```

Then enable hooks:

```bash
prek install
```

Run manually:

```bash
prek run --all-files
```

Hooks run:
- `cargo test`
- `cargo clippy`

## Provider Configuration

Looprs supports multiple LLM providers. Choose based on your needs:

### Anthropic (Recommended)

Fastest to set up. Claude 3 models are excellent for coding tasks.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export MODEL="claude-3-opus-20240229"  # default if not set
looprs
```

**Recommended models:**
- `claude-3-opus-20240229` - Best quality, slower
- `claude-3-sonnet-20240229` - Balanced (recommended)
- `claude-3-haiku-20240307` - Fastest, still capable

### OpenAI

Use GPT-4 or GPT-5 models for maximum capability.

```bash
export OPENAI_API_KEY="sk-..."
export MODEL="gpt-4"  # or "gpt-4-turbo", "gpt-5", etc.
looprs
```

**Recommended models:**
- `gpt-4` - Most capable, highest cost
- `gpt-4-turbo` - Fast and capable
- `gpt-4-32k` - For longer context
- `gpt-5` - Latest (if available)

### Local Ollama

Run open-source models locally. No API costs, full privacy.

```bash
# Terminal 1: Start Ollama
ollama serve

# Terminal 2: Run looprs (auto-detects Ollama)
export MODEL="llama2"  # or "mistral", "neural-chat", etc.
looprs
```

**Installation:** [https://ollama.ai](https://ollama.ai)

**Recommended models:**
- `llama2` - Solid general-purpose model
- `mistral` - Fast and capable
- `neural-chat` - Good for conversations
- `codeup` - Optimized for coding

**Limitations:** Local models don't support function calling (tool use) yet. Looprs gracefully degrades to text-only interaction.

### Provider Selection

Auto-detection priority:
1. `ANTHROPIC_API_KEY` - Anthropic provider
2. `OPENAI_API_KEY` - OpenAI provider
3. Local Ollama on `localhost:11434`

Force a specific provider:

```bash
export PROVIDER="anthropic"  # or "openai", "local"
looprs
```

## Roadmap

### Current Tools
- **read** - Read files with line number pagination
- **write** - Create or overwrite files (auto-creates parent directories)
- **edit** - Replace text in files (with safety checks for ambiguous patterns)
- **glob** - Find files by name patterns (10-100x faster with `fd` installed)
- **grep** - Search file contents with regex (10-100x faster with `rg` installed)
- **bash** - Execute shell commands

### Planned Improvements
- [x] Replace `grep` with `rg` (ripgrep) - **DONE** - grep tool uses rg internally when available
- [x] Add `fd` support - **DONE** - glob tool uses fd internally when available
- [x] Support for multiple language models - **DONE** - Anthropic, OpenAI, Local (Ollama)
- [ ] Performance benchmarks for agent operations
- [ ] Better error recovery and user feedback
- [ ] Session persistence and conversation history
- [ ] Custom tool plugins system
- [ ] OpenRouter provider support

### Why These Tools?
These tools are selected for **speed and correctness**. We avoid UI-focused tools (fzf, lsd) in favor of tools that make the agent's operations faster and more reliable.

### Performance & Graceful Degradation

The grep and glob tools are designed with **progressive enhancement**:

- **With `rg` and `fd` installed**: 10-100x faster searching and file discovery
- **Without these tools**: Falls back to pure Rust implementations automatically
- **No configuration needed**: Detection is automatic - install the tools and you get the speedup
- **Zero breaking changes**: API and output format remain identical

This design ensures looprs performs optimally in any environment while remaining dependency-free.
