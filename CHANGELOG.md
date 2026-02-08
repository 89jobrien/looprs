# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Makefile with common development operations (`make build`, `make test`, `make lint`, `make install`, etc.).
- Comprehensive README with app description, installation instructions, and setup guide.

## [0.1.1] - 2026-02-07

### Added
- CI workflow running `cargo test` and `cargo clippy` on pull requests.
- `bacon.toml` for local dev jobs.
- Documentation for `prek`, `bacon`, and `rust-analyzer`.

### Changed
- Refactored `src/main.rs` into focused modules.
- Updated dependencies and configuration for the new module layout.

## [0.1.0] - 2026-02-07

### Added
- Initial project setup.
