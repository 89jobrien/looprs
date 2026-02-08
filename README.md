# looprs

A concise coding assistant REPL powered by language models. Looprs provides an interactive command-line interface for quick coding tasks, refactoring, and development assistance.

## Getting Started

### Prerequisites

- Rust 1.88 or later
- An API key for your language model service (set via `LOOPRS_API_KEY` environment variable)

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
export LOOPRS_API_KEY="your-api-key-here"
./target/release/looprs
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

## Roadmap

### Current Tools
- **read** - Read files with line number pagination
- **write** - Create or overwrite files (auto-creates parent directories)
- **edit** - Replace text in files (with safety checks for ambiguous patterns)
- **glob** - Find files by name patterns
- **grep** - Search file contents with regex
- **bash** - Execute shell commands

### Planned Improvements
- [ ] Replace `grep` with `rg` (ripgrep) - 10-100x faster pattern matching
- [ ] Add `fd` - Fast alternative to `find` for locating files
- [ ] Performance benchmarks for agent operations
- [ ] Better error recovery and user feedback
- [ ] Support for multiple language models (Claude, GPT, etc.)
- [ ] Session persistence and conversation history
- [ ] Custom tool plugins system

### Why These Tools?
These tools are selected for **speed and correctness**. We avoid UI-focused tools (fzf, lsd) in favor of tools that make the agent's operations faster and more reliable.
