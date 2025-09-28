# Solana Transaction Bundler Makefile

# Variables
RUST_VERSION ?= stable
CARGO_FLAGS ?= --all-features
TARGET_DIR ?= target
DOCKER_IMAGE ?= solana-bundler
DOCKER_TAG ?= latest

# Default target
.PHONY: all
all: check test build

# Development targets
.PHONY: setup
setup: ## Install development dependencies
	@echo "Setting up development environment..."
	rustup install $(RUST_VERSION)
	rustup default $(RUST_VERSION)
	rustup component add rustfmt clippy llvm-tools-preview
	cargo install cargo-audit cargo-deny cargo-llvm-cov criterion
	@echo "Development environment ready!"

.PHONY: check
check: ## Run all checks (format, clippy, audit)
	@echo "Running format check..."
	cargo fmt --all -- --check
	@echo "Running clippy..."
	cargo clippy --all-targets $(CARGO_FLAGS) -- -D warnings
	@echo "Running security audit..."
	cargo audit
	@echo "Running cargo deny..."
	cargo deny check

.PHONY: fmt
fmt: ## Format code
	cargo fmt --all

.PHONY: clippy
clippy: ## Run clippy lints
	cargo clippy --all-targets $(CARGO_FLAGS) -- -D warnings

.PHONY: audit
audit: ## Run security audit
	cargo audit

# Build targets
.PHONY: build
build: ## Build all binaries
	cargo build $(CARGO_FLAGS)

.PHONY: build-release
build-release: ## Build release binaries
	cargo build --release $(CARGO_FLAGS)

.PHONY: build-cli
build-cli: ## Build CLI binary only
	cargo build --bin bundler $(CARGO_FLAGS)

.PHONY: build-service
build-service: ## Build service binary only
	cargo build --bin bundler-service $(CARGO_FLAGS)

# Test targets
.PHONY: test
test: ## Run all tests
	cargo test $(CARGO_FLAGS) --verbose

.PHONY: test-unit
test-unit: ## Run unit tests only
	cargo test --lib $(CARGO_FLAGS) --verbose

.PHONY: test-integration
test-integration: ## Run integration tests only
	cargo test --test '*' $(CARGO_FLAGS) --verbose

.PHONY: test-doc
test-doc: ## Run documentation tests
	cargo test --doc $(CARGO_FLAGS) --verbose

.PHONY: coverage
coverage: ## Generate test coverage report
	cargo llvm-cov --all-features --workspace --html
	@echo "Coverage report generated in target/llvm-cov/html/index.html"

# Benchmark targets
.PHONY: bench
bench: ## Run benchmarks
	cargo bench $(CARGO_FLAGS)

.PHONY: bench-baseline
bench-baseline: ## Run benchmarks and save as baseline
	cargo bench $(CARGO_FLAGS) -- --save-baseline main

.PHONY: bench-compare
bench-compare: ## Compare benchmarks against baseline
	cargo bench $(CARGO_FLAGS) -- --baseline main

# Documentation targets
.PHONY: doc
doc: ## Generate documentation
	cargo doc --no-deps --open $(CARGO_FLAGS)

.PHONY: doc-private
doc-private: ## Generate documentation including private items
	cargo doc --no-deps --document-private-items --open $(CARGO_FLAGS)

# Docker targets
.PHONY: docker-build
docker-build: ## Build Docker image
	docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) .

.PHONY: docker-run
docker-run: ## Run Docker container
	docker run -p 8080:8080 -p 9090:9090 $(DOCKER_IMAGE):$(DOCKER_TAG)

.PHONY: docker-compose-up
docker-compose-up: ## Start all services with docker-compose
	docker-compose up -d

.PHONY: docker-compose-down
docker-compose-down: ## Stop all services
	docker-compose down

.PHONY: docker-compose-logs
docker-compose-logs: ## Show docker-compose logs
	docker-compose logs -f

# Installation targets
.PHONY: install
install: build-release ## Install binaries to system
	cargo install --path crates/bundler-cli --force
	cargo install --path crates/bundler-service --force

.PHONY: install-cli
install-cli: ## Install CLI binary only
	cargo install --path crates/bundler-cli --force

.PHONY: install-service
install-service: ## Install service binary only
	cargo install --path crates/bundler-service --force

# Clean targets
.PHONY: clean
clean: ## Clean build artifacts
	cargo clean

.PHONY: clean-all
clean-all: clean ## Clean everything including Docker
	docker system prune -f
	docker volume prune -f

# Development workflow targets
.PHONY: dev
dev: ## Start development environment
	cargo watch -x "check" -x "test --lib" -x "run --bin bundler-service"

.PHONY: dev-cli
dev-cli: ## Development mode for CLI
	cargo watch -x "check" -x "test --lib" -x "build --bin bundler"

.PHONY: dev-service
dev-service: ## Development mode for service
	cargo watch -x "check" -x "test --lib" -x "run --bin bundler-service"

# Release targets
.PHONY: release-check
release-check: ## Check if ready for release
	@echo "Checking release readiness..."
	cargo fmt --all -- --check
	cargo clippy --all-targets $(CARGO_FLAGS) -- -D warnings
	cargo test $(CARGO_FLAGS)
	cargo audit
	cargo build --release $(CARGO_FLAGS)
	@echo "Release checks passed!"

.PHONY: release-build
release-build: ## Build release artifacts
	@echo "Building release artifacts..."
	cargo build --release $(CARGO_FLAGS)
	mkdir -p release
	cp target/release/bundler release/
	cp target/release/bundler-service release/
	cp README.md release/
	cp examples/bundler.config.toml release/
	cp examples/bundle_request.json release/
	tar -czf release/solana-bundler-$(shell uname -m).tar.gz -C release .
	@echo "Release artifacts created in release/"

# Utility targets
.PHONY: deps
deps: ## Show dependency tree
	cargo tree

.PHONY: deps-outdated
deps-outdated: ## Check for outdated dependencies
	cargo outdated

.PHONY: size
size: ## Show binary sizes
	@echo "Binary sizes:"
	@ls -lh target/release/bundler* 2>/dev/null || echo "No release binaries found. Run 'make build-release' first."

.PHONY: loc
loc: ## Count lines of code
	@echo "Lines of code:"
	@find crates -name "*.rs" | xargs wc -l | tail -1

# Example targets
.PHONY: example-simulate
example-simulate: build-cli ## Run simulation example
	./target/debug/bundler simulate examples/bundle_request.json --verbose

.PHONY: example-health
example-health: ## Check service health
	curl -s http://localhost:8080/v1/health | jq .

.PHONY: example-info
example-info: ## Get service info
	curl -s http://localhost:8080/v1/info | jq .

# Help target
.PHONY: help
help: ## Show this help message
	@echo "Solana Transaction Bundler - Available targets:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Variables:"
	@echo "  RUST_VERSION    Rust version to use (default: stable)"
	@echo "  CARGO_FLAGS     Additional cargo flags (default: --all-features)"
	@echo "  DOCKER_IMAGE    Docker image name (default: solana-bundler)"
	@echo "  DOCKER_TAG      Docker image tag (default: latest)"
	@echo ""
	@echo "Examples:"
	@echo "  make build CARGO_FLAGS='--release'"
	@echo "  make test RUST_VERSION=nightly"
	@echo "  make docker-build DOCKER_TAG=v0.1.0"
