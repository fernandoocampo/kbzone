# kbzone — AI Agent Guide

## What This Project Does

`kbzone` is a synchronous Rust CLI for managing a personal knowledge base stored in a
local SQLite file. The binary is named `kb`.

## Make Targets

- `make build`     — `cargo build --release && cp …` — Compile release binary → `bin/kb`
- `make test`      — `cargo test` — Run all unit + integration tests
- `make lint`      — `cargo clippy -- -D warnings` — Lint (deny warnings)
- `make fmt`       — `cargo fmt` — Auto-format source
- `make fmt-check` — `cargo fmt -- --check` — Check formatting (CI)
- `make install`   — `cargo install --path .` — Install to `~/.cargo/bin`
- `make check`     — fmt-check + lint + test — Full CI gate
- `make clean`     — `cargo clean && rm -rf bin` — Remove build artifacts

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

Always use `make` targets — never invoke `cargo` directly. The Makefile is the single source of truth for how to build, test, and lint this project.

## AI Workflow Rules

### Plan First
- Plan before writing any code — even for trivial tasks.
- If something goes sideways during implementation, **STOP** and re-plan immediately.
  Do not push through with a broken approach.

### Format After Every Code Change
- Run `make fmt` immediately after writing or editing any Rust source file.
- Never rely on `make check` to catch formatting issues — fix them before the CI gate.
- Workflow: write code → `make fmt` → `make check`.

### Verification Before Marking Done
- Never mark a task as done without verifying it works.
- Always run `make check` (or at minimum `make test`) before closing a task.
- A task is only done when tests pass and linter is clean.

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

## Idiomatic Rust Rules

- **Error context** — `Error` variants wrapping I/O or storage failures must carry a `String` (e.g. `GetKBError(String)`). Always use `map_err(|e| Error::Foo(e.to_string()))` — never `map_err(|_| Error::Foo)`.
- **No `PartialEq` on `Error`** — `Error` does not derive `PartialEq`. Tests assert on error variants with `matches!()`: `assert!(matches!(result, Err(Error::SomeVariant)))`.
- **Standard conversion traits** — use `impl From<A> for B` instead of custom `to_b(self)` methods. Callers use `B::from(a)` or `a.into()`.
- **`Display` not side-effecting methods** — domain structs must not call `println!` or any I/O. Implement `std::fmt::Display` and let callers use `print!("{}", value)`.
- **No I/O in the domain layer** — `domain/` has zero dependencies on `std::io`, `println!`, or adapters. All output lives in `cli/handlers.rs`.
- **Two kinds of row helpers — choose the right one**:
  - Helpers used with `query_map` or `conn.query_row` **must** return `rusqlite::Result<T>` (e.g. `row_to_kb_item`). These can be passed directly as the row closure.
  - Helpers that map column errors to domain `Error` (e.g. `row_to_kb`) return `Result<T, Error>` and **cannot** be passed to `query_row`/`query_map`. Use `prepare()` + `stmt.query([])` + `rows.next()` instead, then call the helper on the `&Row` manually.
  - Never pass individual column values as separate arguments — always take `&rusqlite::Row`.
- **Module visibility** — internal-only modules (`adapters`, `cli`, `service`) use `pub(crate) mod`. Public-API modules (`domain`, `errors`, `ports`) and the binary entry-point module (`application`) use `pub mod`.
- **Test-only constructors** — functions only needed in tests (e.g. `in_memory()`) must carry `#[cfg(test)]`.

## Common Mistakes to Avoid

These are structural violations that agents commonly introduce. Check before submitting.

- **Putting I/O in the domain layer** — `domain/` must have zero `println!`, `eprintln!`,
  or `std::io` usage. All output belongs in `cli/handlers.rs`.
- **Adding a third bare parameter to a function** — if you need more than 2 args
  (excluding `self`), define a struct. No exceptions.
- **Using `unwrap()` in library code** — use `?` or `expect("descriptive reason")`.
- **Deriving `PartialEq` on `Error`** — forbidden. Use `matches!()` in tests.
- **Inline SQL string literals** — all SQL must be `const &str`, never an inline `"SELECT…"`.
- **Reaching across layer boundaries** — e.g. importing adapter concrete types in `service/`
  or `cli/`. Always depend on traits, not implementations (except `application/app.rs`).
- **Discarding error context** — never `map_err(|_| Error::Foo)`. Always
  `map_err(|e| Error::Foo(e.to_string()))`.
- **Making `#[cfg(test)]` constructors public in production** — test-only helpers must carry
  `#[cfg(test)]`.

## CLI Commands

- `kb add`                     — Add a new entry (also indexes embedding)
- `kb get`                     — Fetch a single entry by key or ID
- `kb list`                    — List entries with optional filters
- `kb update`                  — Update an entry (also re-indexes embedding)
- `kb delete`                  — Delete an entry (also removes embedding)
- `kb search --keyword <term>` — FTS5 tag search
- `kb ask "<query>"`           — Semantic / vector search (natural language)
- `kb reindex`                 — Rebuild embeddings for all entries
- `kb quote`                   — Print a random quote-category entry

## Configuration

Config file: `~/kbzona/config.yaml` (or `$KBZONA_HOME/config.yaml`).

```yaml
db_path: ~/.kbzona/kbzona.db
embedding:
  provider: fastembed   # default; only supported value for now
```
