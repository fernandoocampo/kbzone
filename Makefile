.PHONY: build build-version test clean lint fmt fmt-check install check help

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

## fmt: format source code
fmt:
	cargo fmt

## fmt-check: check formatting without modifying files
fmt-check:
	cargo fmt -- --check

## install: install the binary to ~/.cargo/bin
install:
	cargo install --path .

## check: run fmt-check + lint + test (CI gate)
check: fmt-check lint test

## help: list available targets
help:
	@grep -E '^## ' Makefile | sed 's/## //' | column -t -s ':'
