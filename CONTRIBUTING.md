# Contributing to looprs

Thanks for your interest in contributing! This guide explains how to propose
changes and what we expect in pull requests.

## Quick Start

1. Fork the repo and create a feature branch.
2. Make your changes with tests where appropriate.
3. Run the quality gates:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo nextest run --workspace`
   - `cargo xtask check pre-push`
4. Open a pull request with a clear description.

## Development Setup

- Rust 1.88+ is required.
- `cargo-nextest` is required to run the test suite.
- Optional tools:
  - `bacon` for watch mode
  - `prek` for pre-commit hooks

See `README.md` for setup and usage details.

## Coding Guidelines

- Use `anyhow::Result` for fallible functions.
- Add error context with `.context()` / `.with_context()`.
- Keep `crates/looprs-cli`/`crates/looprs-tui` thin; core logic belongs in `crates/looprs`.
- Prefer small, focused modules.

## Tests

- Unit tests live next to the code, in each crate's `src/`.
- Integration tests live in `tests/`.
- Add tests for new behavior and bug fixes.

## Pull Request Checklist

- [ ] Tests added/updated
- [ ] Formatting, clippy, nextest, and `cargo xtask check pre-push` pass
- [ ] Docs updated if behavior changes
- [ ] No secrets committed

## Releases

For maintainers creating releases:

1. Verify a clean release branch and run the full local gates listed above.
2. Review changes since the latest `v*` tag and confirm the semantic version bump.
3. Run `taskit release patch`, `taskit release minor`, or `taskit release major`.
4. Regenerate `CHANGELOG.md` with `git cliff --config cliff.toml --output CHANGELOG.md`.
5. Re-run the full gates, commit the manifests, lockfile, and changelog with a signed
   `chore(release): prepare X.Y.Z` commit, then merge it to `main` through a pull request.
6. Dispatch `gh workflow run release.yml --ref main -f version=X.Y.Z`.
7. Verify all five crates, the `vX.Y.Z` tag, provenance attestations, checksums, SBOMs,
   and GitHub release assets.

The release workflow never changes versions. It validates the merged release commit,
publishes crates with a temporary OIDC credential, waits for registry propagation, and
only then creates the tag and GitHub release. Configure a crates.io trusted publisher for
each publishable crate with repository `89jobrien/looprs`, workflow `release.yml`, and
environment `release`; no long-lived crates.io token is stored in GitHub.

## License

By contributing, you agree that your contributions will be licensed under the
project's MIT license.
