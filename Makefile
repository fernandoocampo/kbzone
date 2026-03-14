.PHONY: build test clean lint fmt fmt-check install check help

BIN_DIR := bin
BINARY  := $(BIN_DIR)/kb

## build: compile release binary → bin/kb
build:
	cargo build --release
	mkdir -p $(BIN_DIR)
	cp target/release/kb $(BINARY)

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
