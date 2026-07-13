#!/usr/bin/env bash
set -euo pipefail

# Nushell (used by xtask hooks and scripts)
cargo install nu --locked

# CI tools matching .github/workflows/ci.yml
cargo install taskit --locked
cargo install cargo-nextest --locked
cargo install cargo-deny --locked
cargo install cargo-machete --locked
cargo install cargo-llvm-cov --locked

# Nightly toolchain for coverage (cargo-llvm-cov requires it)
rustup toolchain install nightly
rustup component add llvm-tools-preview --toolchain nightly
rustup component add llvm-tools-preview  # stable too

echo "looprs devcontainer ready."
