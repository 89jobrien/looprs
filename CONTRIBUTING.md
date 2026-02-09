# Contributing to looprs

Thanks for your interest in contributing! This guide explains how to propose
changes and what we expect in pull requests.

## Quick Start

1. Fork the repo and create a feature branch.
2. Make your changes with tests where appropriate.
3. Run the quality gates:
   - `make fmt`
   - `make lint`
   - `make test`
   - `make build`
4. Open a pull request with a clear description.

## Development Setup

- Rust 1.88+ is required.
- Optional tools:
  - `bacon` for watch mode
  - `prek` for pre-commit hooks

See `README.md` for setup and usage details.

## Coding Guidelines

- Use `anyhow::Result` for fallible functions.
- Add error context with `.context()` / `.with_context()`.
- Keep the CLI thin; core logic belongs in `src/`.
- Prefer small, focused modules.

## Tests

- Unit tests live next to the code in `src/`.
- Integration tests live in `tests/`.
- Add tests for new behavior and bug fixes.

## Pull Request Checklist

- [ ] Tests added/updated
- [ ] `make fmt`, `make lint`, `make test`, `make build` all pass
- [ ] Docs updated if behavior changes
- [ ] No secrets committed

## License

By contributing, you agree that your contributions will be licensed under the
project's MIT license.
