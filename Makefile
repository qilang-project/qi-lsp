# Makefile for Qi Language Server

.PHONY: help build test clean install lint fmt docs release dev

# Default target
help:
	@echo "Qi Language Server Build System"
	@echo ""
	@echo "Available targets:"
	@echo "  build     - Build the language server (debug)"
	@echo "  test      - Run all tests"
	@echo "  clean     - Clean build artifacts"
	@echo "  install   - Install to local cargo bin"
	@echo "  lint      - Run linter checks"
	@echo "  fmt       - Format code"
	@echo "  docs      - Generate documentation"
	@echo "  release   - Build optimized release version"
	@echo "  dev       - Start development server with logging"
	@echo "  help      - Show this help message"

# Build targets
build:
	@echo "Building Qi Language Server..."
	cargo build

release:
	@echo "Building Qi Language Server (release)..."
	cargo build --release

# Test targets
test:
	@echo "Running tests..."
	cargo test

test-verbose:
	@echo "Running tests with verbose output..."
	cargo test -- --nocapture

# Development targets
dev:
	@echo "Starting development server with debug logging..."
	RUST_LOG=debug QI_LSP_DEBUG=1 cargo run

dev-release:
	@echo "Starting release server with debug logging..."
	RUST_LOG=debug QI_LSP_DEBUG=1 cargo run --release

# Installation targets
install: build
	@echo "Installing Qi Language Server..."
	cargo install --path .

install-release: release
	@echo "Installing Qi Language Server (release)..."
	cargo install --path . --release

# Code quality targets
lint:
	@echo "Running linter checks..."
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	@echo "Formatting code..."
	cargo fmt

fmt-check:
	@echo "Checking code formatting..."
	cargo fmt -- --check

# Documentation targets
docs:
	@echo "Generating documentation..."
	cargo doc --no-deps

docs-open: docs
	@echo "Opening documentation in browser..."
	cargo doc --no-deps --open

# Cleanup targets
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

clean-all: clean
	@echo "Cleaning additional files..."
	rm -rf target/

# Utility targets
check:
	@echo "Running all checks..."
	cargo check
	cargo test
	cargo clippy
	cargo fmt -- --check

bench:
	@echo "Running benchmarks..."
	cargo bench

coverage:
	@echo "Generating test coverage..."
	cargo tarpaulin --out Html

# Docker targets (optional)
docker-build:
	@echo "Building Docker image..."
	docker build -t qi-lsp:latest .

docker-run:
	@echo "Running Qi Language Server in Docker..."
	docker run --rm -i qi-lsp:latest

# Version management
version:
	@echo "Current version: $$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"

version-patch:
	@echo "Bumping patch version..."
	cargo bump patch

version-minor:
	@echo "Bumping minor version..."
	cargo bump minor

version-major:
	@echo "Bumping major version..."
	cargo bump major

# Publishing (for crates.io)
publish-dry-run:
	@echo "Dry run publish..."
	cargo publish --dry-run

publish:
	@echo "Publishing to crates.io..."
	cargo publish

# Development utilities
watch:
	@echo "Watching for changes and rebuilding..."
	cargo watch -x run

size:
	@echo "Analyzing binary size..."
	cargo size --bin qi-lsp

audit:
	@echo "Running security audit..."
	cargo audit

update:
	@echo "Updating dependencies..."
	cargo update

outdated:
	@echo "Checking for outdated dependencies..."
	cargo outdated

# CI/CD helpers
ci: lint test
	@echo "CI checks passed"

# Examples and testing
example-test:
	@echo "Testing with example Qi files..."
	@if [ -d "../qi/examples" ]; then \
		for file in ../qi/examples/**/*.qi; do \
			echo "Testing: $$file"; \
			RUST_LOG=info cargo run -- --check "$$file" || echo "Failed to process: $$file"; \
		done \
	else \
		echo "Example directory not found at ../qi/examples"; \
	fi

# Performance profiling
profile:
	@echo "Running with profiling..."
	cargo run --features profiling

memory-profile:
	@echo "Running memory profiling..."
	valgrind --tool=massif cargo run