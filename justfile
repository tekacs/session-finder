# Build the session-finder binary
build:
    cargo build --release

# Run the session-finder with arguments
run *args:
    cargo run --release -- {{args}}

# Install the binary to a local bin directory
install: build
    mkdir -p ../bin
    cp target/release/session-finder ../bin/

# Clean build artifacts
clean:
    cargo clean

# Run tests
test:
    cargo test

# Build and install in one step
setup: build install

# Show help for the session-finder tool
help:
    cargo run --release -- --help

# Example searches
example-rust:
    cargo run --release -- rust error handling

example-project PROJECT:
    cargo run --release -- --project {{PROJECT}} implementation

# Build optimized release binary
release:
    cargo build --release --target-cpu=native