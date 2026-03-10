# FMRL Justfile
# Quick commands for building, testing, and developing the FMRL codec
#
# Install just: https://github.com/casey/just
#
# Common commands:
#   just build      - Build the Rust library
#   just test       - Run all tests
#   just wasm       - Build WebAssembly module
#   just serve      - Serve web demo locally
#   just clean      - Clean build artifacts
#   just all        - Build and test everything

# Default recipe - show available commands
default:
    @just --list

# =============================================================================
# BUILD COMMANDS
# =============================================================================

# Build the Rust library (native target)
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Build WebAssembly module for web target
wasm:
    wasm-pack build --target web --features wasm

# Build WebAssembly in release mode
wasm-release:
    wasm-pack build --target web --release

# Build everything (native + wasm)
build-all: build wasm

# =============================================================================
# TEST COMMANDS
# =============================================================================

# Run all tests
test:
    cargo test

# Run tests with output visible
test-verbose:
    cargo test -- --nocapture

# Run a specific test by name
test-one TEST_NAME:
    cargo test {{TEST_NAME}}

# Run roundtrip tests only
test-roundtrip:
    cargo test roundtrip

# Run decay determinism tests
test-decay:
    cargo test decay

# Run chunk parsing tests
test-chunk:
    cargo test chunk

# Run age mutation tests
test-age:
    cargo test age

# Run all tests and generate coverage (requires cargo-tarpaulin)
test-coverage:
    cargo tarpaulin --out Html

# =============================================================================
# LINT & CHECK COMMANDS
# =============================================================================

# Run clippy lints
check:
    cargo clippy -- -D warnings

# Run clippy with all features enabled
check-all:
    cargo clippy --all-features -- -D warnings

# Format code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt -- --check

# Run full CI checks (lint, format check, test)
ci: fmt-check check test

# =============================================================================
# WEB DEMO COMMANDS
# =============================================================================

# Serve the web demo locally using Python (port 8080)
serve:
    python3 -m http.server 8080 --directory docs/

# Serve using npx serve (alternative)
serve-npx:
    npx serve -l 8080 docs/

# Open the web demo in browser (macOS)
open-demo: wasm
    open http://localhost:8080

# Build and serve (convenient for development)
dev: wasm serve

# Stop/halt the running demo server
halt:
    @echo "Stopping demo server on port 8080..."
    lsof -ti:8080 | xargs -r kill -9 2>/dev/null || echo "No server running on port 8080"

# Deploy the web app: build WASM, copy to docs, and serve
deploy:
    @echo "Building WASM module..."
    wasm-pack build --target web --features wasm
    @echo "Copying to docs..."
    cp -r pkg docs/
    @echo "Starting server on http://localhost:8080"
    python3 -m http.server 8080 --directory docs/

# Sync theme palettes from fmrl.toml to docs/themes.json
sync-themes:
    @echo "Syncing themes from fmrl.toml to docs/themes.json..."
    python3 sync_themes.py

# Full deploy with theme sync
deploy-all: sync-themes wasm
    @echo "Copying to docs..."
    cp -r pkg docs/
    @echo "Starting server on http://localhost:8080"
    python3 -m http.server 8080 --directory docs/

# =============================================================================
# CLEANUP COMMANDS
# =============================================================================

# Clean build artifacts
clean:
    cargo clean
    rm -rf pkg/

# Clean everything including wasm build artifacts
clean-all: clean
    rm -rf wasm/pkg/
    rm -rf target/

# =============================================================================
# DOCUMENTATION COMMANDS
# =============================================================================

# Generate and open documentation
docs:
    cargo doc --open

# Generate docs for all features
docs-all:
    cargo doc --all-features --open

# =============================================================================
# UTILITY COMMANDS
# =============================================================================

# Update dependencies
update:
    cargo update

# Check for outdated dependencies (requires cargo-outdated)
outdated:
    cargo outdated

# Run security audit (requires cargo-audit)
audit:
    cargo audit

# Build and run a quick smoke test
smoke: build test-roundtrip
    @echo "✓ Smoke test passed"

# Full build, test, and check cycle
all: fmt check build test wasm
    @echo "✓ All tasks completed successfully"

# =============================================================================
# RELEASE COMMANDS
# =============================================================================

# Prepare a release (build, test, verify)
release: all
    @echo "✓ Release build ready"

# Publish to crates.io (requires auth)
publish:
    cargo publish

# Create a new version tag
version VERSION:
    git tag -a v{{VERSION}} -m "Release v{{VERSION}}"
    git push origin v{{VERSION}}
