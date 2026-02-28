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

ui:
	cargo run -p looprs-desktop

all: check lint test

# Run all tests with nextest (fast parallel execution)
test-all:
	cargo nextest run

# Run tests with coverage report
test-coverage:
	cargo llvm-cov nextest --html
	@echo "Coverage report: target/llvm-cov/html/index.html"

# Run only desktop UI tests
test-ui:
	cargo nextest run -p looprs-desktop --test ui

# Run only database tests
test-db:
	cargo nextest run -p looprs-desktop sqlite_store

# Run only service layer tests
test-services:
	cargo nextest run -p looprs-desktop --lib generative_ui mockstation

# Run property tests
test-property:
	cargo nextest run property

# Run integration tests
test-integration:
	cargo nextest run -p looprs-desktop --test desktop_flow

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
