# looprs

## Development

### Rust-analyzer

Recommended for editor diagnostics and code navigation:

```bash
rustup component add rust-analyzer
```

### Bacon

`bacon` provides fast local feedback loops using the jobs in `bacon.toml`.

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
