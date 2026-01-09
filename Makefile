.PHONY: help build test clean bench fmt lint check docs run-example docker-up docker-down

# Default target
.DEFAULT_GOAL := help

# Help target
help: ## Show this help message
	@echo "cache-layer - Multi-tier caching library"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# Build targets
build: ## Build the project (debug)
	cargo build

build-release: ## Build the project (release)
	cargo build --release

build-all: ## Build with all features
	cargo build --all-features

# Test targets
test: ## Run all tests
	cargo test --all

test-unit: ## Run unit tests only
	cargo test --lib

test-integration: ## Run integration tests
	cargo test --test '*'

test-with-coverage: ## Run tests with coverage
	cargo tarpaulin --out Html --output-dir coverage/

test-watch: ## Run tests in watch mode
	cargo watch -x test

# Benchmark targets
bench: ## Run all benchmarks
	cargo bench

bench-memory: ## Run memory benchmarks
	cargo bench --bench memory_bench

bench-redis: ## Run Redis benchmarks
	cargo bench --bench redis_bench

bench-disk: ## Run disk benchmarks
	cargo bench --bench disk_bench

bench-multi: ## Run multi-tier benchmarks
	cargo bench --bench multi_tier_bench

bench-save: ## Save benchmark results
	cargo bench -- --save-baseline main

bench-compare: ## Compare with saved baseline
	cargo bench -- --baseline main

# Code quality targets
fmt: ## Format code
	cargo fmt

fmt-check: ## Check code formatting
	cargo fmt -- --check

lint: ## Run linter
	cargo clippy --all-targets --all-features

lint-fix: ## Fix linter warnings
	cargo clippy --fix --all-targets --all-features

check: fmt-check lint ## Run all checks

# Documentation targets
docs: ## Generate documentation
	cargo doc --no-deps --all-features

docs-open: ## Generate and open documentation
	cargo doc --no-deps --all-features --open

# Clean targets
clean: ## Clean build artifacts
	cargo clean

clean-docker: ## Clean Docker resources
	docker-compose down -v

clean-all: clean clean-docker ## Clean everything

# Development targets
run-example: ## Run basic usage example
	cargo run --example basic_usage

run-vector: ## Run vector navigator example
	cargo run --example vector_navigator

run-monitoring: ## Run monitoring example
	cargo run --example monitoring

run-ecosystem: ## Run ecosystem integration example
	cargo run --example ecosystem_integration

# Docker targets
docker-up: ## Start test infrastructure (Redis)
	docker-compose up -d redis

docker-down: ## Stop test infrastructure
	docker-compose down

docker-logs: ## View Docker logs
	docker-compose logs -f

docker-monitoring: ## Start monitoring stack
	docker-compose --profile monitoring up -d

# Release targets
release: ## Create release (tag, publish)
	./scripts/release.sh

# Development setup
setup: ## Set up development environment
	@echo "Setting up development environment..."
	@rustup --version || echo "Please install Rust from https://rustup.rs/"
	@cargo --version
	@docker --version || echo "Docker not found (optional, for integration tests)"
	@echo "Running initial build..."
	cargo build
	@echo "Setup complete!"

# CI targets
ci: check test ## Run CI checks (format, lint, test)

ci-all: check test-with-coverage bench ## Run full CI (with coverage and benchmarks)

# Installation targets
install-tools: ## Install development tools
	cargo install cargo-watch
	cargo install cargo-expand
	cargo install cargo-tarpaulin
	cargo install cargo-release
	cargo install flamegraph
