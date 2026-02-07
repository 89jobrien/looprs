# looprs

## Development

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
