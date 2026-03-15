# kbzone — Claude Code Guide

## What This Project Does

`kbzone` is a synchronous Rust CLI for managing a personal knowledge base backed by a
local SQLite file. Entries have a key, value, notes, category, namespace, reference, and
space-separated tags (FTS5-indexed). The compiled binary is called `kb`.

## Quick Commands

```bash
make build        # → bin/kb
make test         # cargo test (unit + integration)
make check        # fmt-check + lint + test  (CI gate)
./bin/kb --help
```

## Architecture Module Map

```
src/
  main.rs                        Trivial entry point
  lib.rs                         Module declarations
  errors/error.rs                Error + AppError enums (thiserror)
  domain/kb.rs                   Domain structs: Kb, NewKb, KbFilter, KbItem,
                                   ScoredKbItem, SemanticQuery, EmbeddingInput
  ports/storage.rs               KbStore trait — outbound port
  ports/embedding.rs             EmbeddingProvider trait — outbound port
  ports/vector_store.rs          VectorStore trait — outbound port
  service/kb_service.rs          Business logic; unit-tested with MockKbStore
  service/semantic_service.rs    SemanticService<V,E>; unit-tested with mocks
  adapters/sqlite/store.rs       SqliteStore implements KbStore + VectorStore;
                                   integration-tested in-memory (sqlite-vec loaded
                                   via sqlite3_auto_extension before each connection)
  adapters/fastembed/provider.rs FastEmbedProvider (BAAI/bge-small-en-v1.5, 384 dims)
  cli/commands.rs                Clap subcommand definitions
  cli/handlers.rs                Handler functions + Services<S,V,E> wrapper
  application/config.rs          Config loaded from ~/kbzona/config.yaml
  application/app.rs             App::build() + App::run()
```

## Hexagonal Architecture — Layer Communication Rules

| From layer | May depend on | Must NOT depend on |
|---|---|---|
| `domain/` | nothing (pure Rust, no crate deps beyond std + serde) | ports, service, adapters, cli, application |
| `ports/` | `domain/` | adapters, service, cli, application |
| `service/` | `domain/`, `ports/` (traits only) | adapters (concrete types), cli, application |
| `adapters/` | `domain/`, `ports/` | service, cli, application |
| `cli/` | `domain/`, `ports/`, `service/` | adapters (concrete types), application |
| `application/` | all layers (wiring only) | — |

### Data flow for a typical command

```
CLI args
  → cli/commands.rs (parse)
  → cli/handlers.rs (orchestrate, I/O)
    → service/kb_service.rs (business logic)
      → ports/storage.rs (trait)
        → adapters/sqlite/store.rs (implementation)
    → service/semantic_service.rs (embedding coordination)
      → ports/embedding.rs + ports/vector_store.rs (traits)
        → adapters/fastembed/provider.rs (implementation)
```

**Key rules:**
- `application/app.rs` is the **composition root**: the only place where concrete adapter types are named. All other layers depend on traits.
- `cli/handlers.rs` coordinates between services but never instantiates adapters directly.
- New types needed for a feature (e.g. `ImportBatchResult`) belong in `domain/` if they carry no I/O or adapter logic.

## Build & Test Commands

Always use `make` targets — never invoke `cargo` directly:

```bash
make build   # compile
make test    # run tests
make check   # full CI gate (fmt-check + lint + test)
```

## Coding Constraints

- **No async** — this is a synchronous CLI; do not add tokio or async/await.
- **No `unwrap` in library code** — prefer `?` or `expect("reason")`.
- **SQL as const** — every SQL statement must be a `const &str`, not an inline literal.
- **Binary name** — always `kb` (set in `[[bin]]` in `Cargo.toml`).
- **Error split** — domain/storage errors → `errors::Error`; startup failures → `errors::AppError`.
- **Storage init** — always call `store.initialize()` once at startup; DDL is idempotent.
- **Function arguments** — functions and methods must have at most 2 parameters (excluding `self`/`&self`). If more data is needed, define a dedicated struct to carry the parameters; do not add a third bare argument under any circumstance.
- **TDD** — always write unit tests before implementing the code logic. Define the test cases first, confirm they fail, then write the minimum code to make them pass.

## Idiomatic Rust Rules

- **Error context** — every `Error` variant that wraps an underlying I/O or storage failure must carry a `String` payload (e.g. `GetKBError(String)`). Never discard the source with `map_err(|_| Error::Foo)` — always use `map_err(|e| Error::Foo(e.to_string()))`.
- **No `PartialEq` on `Error`** — `Error` does not derive `PartialEq`. Tests must use `matches!()` to check error variants: `assert!(matches!(result, Err(Error::SomeVariant)))`.
- **Standard conversion traits** — when a type is fully consumed and returned as another type, implement `From<A> for B` instead of a custom `to_b(self)` method. Callers use `B::from(a)` or `a.into()`.
- **`Display` not side-effecting methods** — domain structs must not call `println!` or any I/O directly. Implement `std::fmt::Display` and let callers use `print!("{}", value)` or `format!`.
- **No I/O in the domain layer** — `domain/` structs and functions must have zero dependencies on `std::io`, `println!`, or any adapter. Output belongs in `cli/handlers.rs`.
- **Row parsers take `&Row`** — SQLite row-mapping helper functions must accept `&rusqlite::Row` and return `rusqlite::Result<T>`, matching the signature expected by `query_map` / `query_row`. Do not accept individual column values as separate arguments.
- **Module visibility** — modules that are only consumed within the library crate (`adapters`, `cli`, `service`) must be declared `pub(crate) mod`. Modules that form the public API (`domain`, `errors`, `ports`) and the binary entry-point module (`application`) stay `pub mod`.
- **Test-only constructors** — functions only needed for tests (e.g. `in_memory()`) must be annotated `#[cfg(test)]` to avoid dead-code warnings in production builds.

## Configuration

Config file: `~/kbzona/config.yaml` (or `$KBZONA_HOME/config.yaml`).

```yaml
db_path: ~/.kbzona/kbzona.db
embedding:
  provider: fastembed   # default; only supported value for now
```

A default config is written automatically if the file is absent.

## Semantic Search

`kb ask "natural language query"` performs vector/semantic search.
`kb reindex` regenerates embeddings for all existing entries.

Key design points:
- `EmbeddingProvider` and `VectorStore` are separate outbound ports (traits).
- `SqliteStore` implements both `KbStore` and `VectorStore` (same DB connection via `Arc<Mutex<Connection>>`).
- `SemanticService<V: VectorStore, E: EmbeddingProvider>` coordinates embedding + search.
- `Services<S, V, E>` in `cli/handlers.rs` groups both services to satisfy the 2-param rule.
- sqlite-vec KNN queries (`vec0` MATCH) **do not support JOINs** — `search_similar` uses two queries: KNN → kb_ids, then a regular `kbs` lookup.
- sqlite-vec extension is registered via `sqlite3_auto_extension` (with `std::sync::Once`) before each `Connection` is opened.
- Embedding failures on `add`/`update` are **non-fatal** — the entry is saved and a warning is printed. Run `kb reindex` to recover.
- `Kb::embedding_text()` builds the embedded text from: key + category + namespace + tags + first 200 chars of value (notes excluded).
