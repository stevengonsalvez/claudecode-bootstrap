# Justfile for Claude-in-a-Box development

# Default recipe to display available commands
default:
    just --list

# Build the project
build:
    cargo build

# Build in release mode
build-release:
    cargo build --release

# Run the application
run *args:
    cargo run -- {{args}}

# Run tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Format code
fmt:
    cargo fmt

# Check formatting
fmt-check:
    cargo fmt --check

# Run clippy
lint:
    cargo clippy -- -D warnings

# Fix clippy issues automatically where possible
lint-fix:
    cargo clippy --fix --allow-dirty --allow-staged

# Check everything (format, lint, test)
check:
    just fmt-check
    just lint
    just test

# Fix formatting and linting issues
fix:
    just fmt
    just lint-fix

# Clean build artifacts
clean:
    cargo clean

# Install development dependencies
setup:
    rustup component add rustfmt clippy
    cargo install just

# Watch and rebuild on changes
watch:
    cargo watch -x check -x test -x run

# Generate documentation
docs:
    cargo doc --open

# Run benchmarks (if any)
bench:
    cargo bench

# Check dependency licenses
audit:
    cargo audit
