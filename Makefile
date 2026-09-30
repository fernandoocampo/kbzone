.PHONY: build build-version test clean lint fmt fmt-check install check help vuln-check vuln-check-docker

BIN_DIR    := bin
BINARY     := $(BIN_DIR)/kb
GIT_HASH   := $(shell git describe --dirty --tags --always)
BUILD_DATE := $(shell date -u +"%Y-%m-%dT%H:%M:%SZ")
VERSION    ?= $(GIT_HASH)

## build: compile release binary → bin/kb
build:
	KB_VERSION=$(VERSION) KB_BUILD_DATE=$(BUILD_DATE) KB_GIT_HASH=$(GIT_HASH) cargo build --release
	mkdir -p $(BIN_DIR)
	cp target/release/kb $(BINARY)

## build-version: compile release binary with explicit version tag (e.g. make build-version VERSION=v1.0.0)
build-version: build

## test: run all tests
test:
	cargo test

## clean: remove build artifacts and bin/
clean:
	cargo clean
	rm -rf $(BIN_DIR)

## lint: run clippy
lint:
	cargo clippy -- -D warnings

## vuln-check: scan dependencies for known vulnerabilities via RustSec advisory DB (requires cargo-audit, e.g. `cargo install cargo-audit --locked`)
vuln-check:
	GIT_CONFIG_GLOBAL=/dev/null cargo audit

## vuln-check-docker: same as vuln-check, but runs in a throwaway rust-slim container (no local cargo-audit install needed)
vuln-check-docker:
	docker run --rm -v "$(CURDIR)":/app -w /app rust:slim sh -c "cargo install cargo-audit --locked && cargo audit"

## fmt: format source code
fmt:
	cargo fmt

## fmt-check: check formatting without modifying files
fmt-check:
	cargo fmt -- --check

## install: install the binary to ~/.cargo/bin
install:
	cargo install --path .

## check: run fmt-check + lint + test + vuln-check (CI gate)
check: fmt-check lint test vuln-check

## help: list available targets
help:
	@grep -E '^## ' Makefile | sed 's/## //' | column -t -s ':'
