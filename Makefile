# Makefile for GitOps Bootstrap TUI

# Variables
BINARY_NAME=gitops-bootstrap-tui
TARGET_MUSL_X86=x86_64-unknown-linux-musl
TARGET_MUSL_ARM=aarch64-unknown-linux-musl

.PHONY: build build-musl-amd64 build-musl-arm64 run clean install

# Standard development build
build:
	cargo build

# Statically linked MUSL build for Linux (x86_64)
build-musl-amd64:
	cargo build --release --target $(TARGET_MUSL_X86)

# Statically linked MUSL build for Linux (ARM64)
build-musl-arm64:
	cargo build --release --target $(TARGET_MUSL_ARM)

# Run the TUI locally
run:
	cargo run

# Run tests
test:
	cargo test

# Format the code
fmt:
	cargo fmt

# Clean the target directory
clean:
	cargo clean

# Install locally via cargo
install:
	cargo install --path .
