# kbzone — AI Agent Guide

## What This Project Does

`kbzone` is a synchronous Rust CLI for managing a personal knowledge base stored in a
local SQLite file. The binary is named `kb`.

## Make Targets

| Target       | Equivalent cargo command            | Purpose                              |
|--------------|-------------------------------------|--------------------------------------|
| `make build` | `cargo build --release && cp …`     | Compile release binary → `bin/kb`    |
| `make test`  | `cargo test`                        | Run all unit + integration tests     |
| `make lint`  | `cargo clippy -- -D warnings`       | Lint (deny warnings)                 |
| `make fmt`   | `cargo fmt`                         | Auto-format source                   |
| `make fmt-check` | `cargo fmt -- --check`          | Check formatting (CI)                |
| `make install` | `cargo install --path .`          | Install to `~/.cargo/bin`            |
| `make check` | fmt-check + lint + test             | Full CI gate                         |
| `make clean` | `cargo clean && rm -rf bin`         | Remove build artifacts               |

## Architecture Module Map

```
src/
  main.rs                        Entry point — App::build().and_then(|a| a.run())
  lib.rs                         Module declarations
  errors/error.rs                Error + AppError (thiserror)
  domain/kb.rs                   Kb, NewKb, KbFilter, KbItem,
                                   ScoredKbItem, SemanticQuery, EmbeddingInput
  ports/storage.rs               KbStore trait (outbound port)
  ports/embedding.rs             EmbeddingProvider trait (outbound port)
  ports/vector_store.rs          VectorStore trait (outbound port)
  service/kb_service.rs          Service<T: KbStore> + unit tests (MockKbStore)
  service/semantic_service.rs    SemanticService<V,E> + unit tests (mocks)
  adapters/sqlite/store.rs       SqliteStore implements KbStore + VectorStore;
                                   integration tests (in-memory + sqlite-vec)
  adapters/fastembed/provider.rs FastEmbedProvider (bge-small-en-v1.5, 384 dims)
  cli/commands.rs                Clap derive subcommands (incl. ask, reindex)
  cli/handlers.rs                Handlers + Services<S,V,E> container struct
  application/config.rs          Config (YAML) — db_path + embedding.provider
  application/app.rs             App::build() wires deps; App::run() dispatches CLI
```

## Build & Test Commands

Always use `make` targets — never invoke `cargo` directly. The Makefile is the single source of truth for how to build, test, and lint this project.

## Coding Constraints

- **No async** — fully synchronous; tokio is not a dependency.
- **No `unwrap` in library code** — use `?` or explicit `expect` with a message.
- **SQL as const** — all SQL strings are `const &str` constants, never inline string literals. Dynamic DDL (e.g. the vec0 dimension) uses a `const` template with `.replace()` at runtime.
- **Binary name is `kb`** — configured via `[[bin]]` in `Cargo.toml`.
- **Error types** — domain errors go in `Error`; startup/config errors go in `AppError`.
- **Storage init** — call `store.initialize()` then `store.initialize_vectors(dims)` once at startup; both are idempotent.
- **Function arguments** — functions and methods must have at most 2 parameters (excluding `self`/`&self`). If more data is needed, define a dedicated struct to carry the parameters; do not add a third bare argument under any circumstance.
- **TDD** — always write unit tests before implementing the code logic. Define the test cases first, confirm they fail, then write the minimum code to make them pass.
- **sqlite-vec** — extension is loaded via `sqlite3_auto_extension` (with `std::sync::Once`) before each `Connection` open; vec0 MATCH queries do not support JOINs — use two queries instead.
- **Semantic search** — embedding failures on `add`/`update` are non-fatal; the entry is always saved. Run `kb reindex` to rebuild missing embeddings.

## CLI Commands

| Command | Description |
|---------|-------------|
| `kb add` | Add a new entry (also indexes embedding) |
| `kb get` | Fetch a single entry by key or ID |
| `kb list` | List entries with optional filters |
| `kb update` | Update an entry (also re-indexes embedding) |
| `kb delete` | Delete an entry (also removes embedding) |
| `kb search --keyword <term>` | FTS5 tag search |
| `kb ask "<query>"` | Semantic / vector search (natural language) |
| `kb reindex` | Rebuild embeddings for all entries |

## Configuration

Config file: `~/kbzona/config.yaml` (or `$KBZONA_HOME/config.yaml`).

```yaml
db_path: ~/.kbzona/kbzona.db
embedding:
  provider: fastembed   # default; only supported value for now
```
