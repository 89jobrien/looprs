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

## Releases

For maintainers creating releases:

1. Use conventional commit messages (`feat:`, `fix:`, `docs:`, etc.) for automatic changelog generation
2. Run version bump script:
   - `make version-patch` for bug fixes (0.1.11 → 0.1.12)
   - `make version-minor` for new features (0.1.11 → 0.2.0)
   - `make version-major` for breaking changes (0.1.11 → 1.0.0)
3. The script automatically:
   - Updates `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md`
   - Categorizes commits into changelog sections
   - Creates git commit and tag
4. Push changes and tag: `git push origin main && git push origin vX.Y.Z`
5. Create GitHub release (optional)

See `scripts/README.md` for detailed documentation.

## License

By contributing, you agree that your contributions will be licensed under the
project's MIT license.
