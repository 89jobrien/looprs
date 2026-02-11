.PHONY: help build test check fmt lint clean install run watch setup version-patch version-minor version-major

CARGO := cargo
RUST_VERSION := 1.88
VERSION_SCRIPT := ./scripts/bump-version.sh

help:
	@echo "looprs - Rust Coding Assistant REPL"
	@echo ""
	@echo "Common commands:"
	@echo "  make build       Build the project in release mode"
	@echo "  make dev         Build the project in debug mode"
	@echo "  make test        Run all tests"
	@echo "  make check       Run cargo check"
	@echo "  make lint        Run clippy linter"
	@echo "  make fmt         Format code with rustfmt"
	@echo "  make fmt-check   Check code formatting without changes"
	@echo "  make clean       Remove build artifacts"
	@echo "  make install     Build and install binary locally"
	@echo "  make run         Run the REPL (requires API key)"
	@echo "  make watch       Watch for changes and run tests (requires bacon)"
	@echo "  make all         Run check, fmt-check, lint, and test"
	@echo ""
	@echo "Release commands:"
	@echo "  make version-patch   Bump patch version (0.1.11 -> 0.1.12)"
	@echo "  make version-minor   Bump minor version (0.1.11 -> 0.2.0)"
	@echo "  make version-major   Bump major version (0.1.11 -> 1.0.0)"
	@echo ""
	@echo "Setup commands:"
	@echo "  make setup       Install dev dependencies (bacon)"
	@echo ""

build:
	$(CARGO) build --release

dev:
	$(CARGO) build

test:
	$(CARGO) test --lib

test-all:
	$(CARGO) test --all-targets

check:
	$(CARGO) check

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

clean:
	$(CARGO) clean
	rm -rf target/

install: build
	$(CARGO) install --path .

run:
	./target/release/looprs

watch:
	bacon check

all: check fmt-check lint test
	@echo "✓ All checks passed!"

setup:
	@echo "Installing dev dependencies..."
	@command -v bacon >/dev/null 2>&1 || cargo install --locked bacon
	@echo "✓ Dev dependencies installed"

verify-rust:
	@$(CARGO) --version | grep -q "$(RUST_VERSION)" || (echo "Rust $(RUST_VERSION)+ required"; exit 1)
	@echo "✓ Rust version OK"

ci: verify-rust check fmt-check lint test-all
	@echo "✓ CI checks passed!"

# Version management commands
version-patch:
	@bash $(VERSION_SCRIPT) patch

version-minor:
	@bash $(VERSION_SCRIPT) minor

version-major:
	@bash $(VERSION_SCRIPT) major
