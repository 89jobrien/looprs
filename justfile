default:
	@just --list

cliff := "git cliff --config cliff.toml"

build:
	cargo build --release

# Build and install the looprs binary to ~/.cargo/bin
install:
	cargo install --path crates/looprs-cli --locked

check:
	cargo check

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test --lib


all: check lint test

# Run all tests with nextest (fast parallel execution)
test-all:
	cargo nextest run

# Run tests with coverage report
test-coverage:
	cargo llvm-cov nextest --html
	@echo "Coverage report: target/llvm-cov/html/index.html"


# Run property tests
test-property:
	cargo nextest run property

# Run benchmarks
bench:
	cargo bench

# Watch tests (requires cargo-watch: cargo install cargo-watch)
test-watch:
	cargo watch -x "nextest run"

# Update all insta snapshots
test-update-snapshots:
	cargo insta test --accept

# Install testing tools
install-test-tools:
	cargo install cargo-nextest --locked
	cargo install cargo-llvm-cov --locked
	cargo install cargo-watch --locked
	cargo install cargo-insta --locked

# Generate unreleased changelog preview from commits.
changelog:
	@echo "==> Unreleased changelog preview"
	{{cliff}} --unreleased --strip all

# Generate changelog for commits since the latest tag.
changelog-since-tag:
	@echo "==> Changelog since latest tag"
	{{cliff}} --latest --strip all

# Regenerate CHANGELOG.md from git history.
changelog-write:
	@echo "==> Writing CHANGELOG.md"
	{{cliff}} --output CHANGELOG.md
	@echo "Wrote CHANGELOG.md"
