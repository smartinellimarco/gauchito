# List all available commands
help:
    @just --list

# Build
build:
    cargo build --workspace

# Run TUI
run *args:
    cargo run -q -p gauchito-cli -- {{ args }}

# Run tests
test:
    cargo test --workspace

# Remove build artifacts
clean:
    cargo clean

# Check formatting
check-format:
    cargo fmt -- --check

# Run clippy lints
lint:
    cargo clippy --workspace -- -W warnings

# Auto-fix clippy + format
fix:
    cargo clippy --workspace --fix --allow-dirty
    cargo fmt
