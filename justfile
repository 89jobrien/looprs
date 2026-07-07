default:
	@just --list

build:
	cargo build --release

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
