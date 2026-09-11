# kbzone — AI Agent Guide

## What This Project Does

`kbzone` is a synchronous Rust CLI for managing a personal knowledge base stored in a
local SQLite file. The binary is named `kb`.

### CLI Commands

- `kb add`                     — Add a new entry (also indexes embedding); flags include `--path` (optional Unix-style path, leading `/` auto-added)
- `kb get`                     — Fetch a single entry by key or ID; displays `path` if set; flags: `--out` (`json`\|`yaml`, optional — default is plain text)
- `kb update`                  — Update an entry (also re-indexes embedding); flags include `--path` (empty string clears the path)
- `kb delete`                  — Delete an entry (also removes embedding)
- `kb search`                  — Search/list entries; flags: `--keyword`, `--category`, `--namespace`, `--tags`, `--reference`, `--limit`, `--offset`; uses FTS5 when `--keyword` is set, otherwise a regular SQL filter
- `kb ask "<query>"`           — Semantic / vector search (natural language); flags: `--limit`, `--threshold` (max distance; default `0.9` — results above this value are excluded)
- `kb reindex`                 — Rebuild embeddings for all entries
- `kb export`                  — Export KB entries to a multi-document YAML file; flags: `--file` (required), `--category`, `--namespace`, `--limit`, `--offset`; parents always appear before children; `Parent` field omitted when parent is not in the filtered set; `Path` field included when set
- `kb import`                  — Import KB entries from a multi-document YAML file; `Path` field is validated and normalised on import
- `kb quote`                   — Print a random quote-category entry
- `kb categories`              — List all distinct, non-empty category values; flags: `--namespace` (optional)
- `kb link <from> <to>`        — Create a directed edge between two entries (each `from`/`to` accepts a key or an internal ID); flags: `--note`
- `kb unlink <from> <to>`      — Remove the edge between two entries (key or ID); errors if no such edge exists
- `kb related <key-or-id>`     — Show one-hop outgoing/incoming relationships; flags: `--direction` (`out`\|`in`\|`both`, default `both`), `--json` (full NOTE text; human output truncates NOTE to 40 chars)
- `kb tree <key-or-id>`        — Transitive relationship traversal via a recursive CTE; flags: `--direction` (`out`\|`in`, default `out`), `--depth` (default `10`), `--json`

### Configuration

Config file: `~/kbzona/config.yaml` (or `$KBZONA_HOME/config.yaml`).
The config file is **auto-created with defaults on first run** if it does not exist.

```yaml
db_path: ~/.kbzona/kbzona.db
embedding:
  provider: fastembed   # default; only supported value for now
```

- `$KBZONA_HOME` overrides the base directory (default: `~/.kbzona/`).
- Embedding model cache: `{db_path_parent}/fastembed_cache/`.

#### Embedding text construction

When indexing an entry (on `add`, `update`, or `reindex`), the text fed to the embedding model is:

```
"{key} {category} {namespace} {tags_as_string} {value[0..200]}"
```

This is constructed by `Kb::embedding_text()` in `domain/kb.rs`.

## KB Entity Fields

The `Kb` struct in `domain/kb.rs` is the canonical entity:

```
// Auto-generated UUID — primary key
id: String
// User-defined identifier — normalized to lowercase on save
key: String
// Main content / answer
value: String
// Extended notes or elaboration
notes: String
// Entry type: quote, bookmark, concept, command, etc.
category: String
// Grouping scope: rust, k8s, personal, etc.
namespace: String
// Source: author, book title, URL, or person's name; included in embedding
reference: String
// Searchable keywords — power FTS5 and are included in embedding
tags: Vec<String>
// Optional Unix-style path (e.g. /personal/cars/engines).
// Leading `/` is auto-added if omitted. Validated on add/update/import.
path: Option<String>
// UUID of parent Kb entry — enables hierarchical relationships
parent: Option<String>
// ISO-8601 creation timestamp — set once on save
created_on: String
```

## Graph Relationships

`kb_edges` is a separate, additive table expressing a directed semantic
relationship between two `kbs` records — distinct from the `parent`/`path`
hierarchy (filing/organisation) and from each other (many-to-many, can be
cyclic). The `KbEdge` struct in `domain/graph.rs` is the canonical entity:

```
// Auto-generated UUID
id: String
// Kb.id this edge points from
from_id: String
// Kb.id this edge points to
to_id: String
// Free text describing why these two entries are connected
note: String
// ISO-8601 creation timestamp
created_on: String
```

`from`/`to` on the CLI accept either a `key` or an internal `id`; `GraphService`
resolves them to `Kb.id` (tries `key` first, falls back to `id`) before
touching storage. `(FROM_KB_ID, TO_KB_ID)` is unique — linking the same pair
twice in the same direction is a `DuplicateEdgeError`; a `kbs_ad_edges` trigger
deletes an entry's edges when it is deleted.

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
  domain/graph.rs                 KbEdge, NewKbEdge, EdgeDirection, and the
                                   related/tree query + output DTOs
  ports/storage.rs               KbStore trait (outbound port)
  ports/embedding.rs             EmbeddingProvider trait (outbound port)
  ports/vector_store.rs          VectorStore trait (outbound port)
  ports/graph.rs                  KbGraph trait (outbound port) — edge storage/traversal
  service/kb_service.rs          KBService<S,V,E,M,F> — unified CRUD + semantic +
                                   media service; unit tests (MockKbStore)
  service/graph_service.rs        GraphService<S,G> — edge add/remove/traversal,
                                   key-or-id resolution; unit tests (MockKbStore, MockKbGraph)
  adapters/sqlite/store.rs       SqliteStore implements KbStore + VectorStore + KbGraph;
                                   integration tests (in-memory + sqlite-vec)
  adapters/fastembed/provider.rs FastEmbedProvider (bge-small-en-v1.5, 384 dims)
  cli/commands.rs                Clap derive subcommands (incl. ask, reindex, link,
                                   unlink, related, tree)
  cli/handlers.rs                Free `handle_*` functions per command, each taking
                                   the relevant service + a `*Params` struct (2-param rule)
  application/config.rs          Config (YAML) — db_path + embedding.provider
  application/app.rs             App::build() wires deps (holds both KBService and
                                   GraphService); App::run() dispatches CLI
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
    → service/kb_service.rs (CRUD + semantic + media business logic)
      → ports/storage.rs (trait)
        → adapters/sqlite/store.rs (implementation)
      → ports/embedding.rs + ports/vector_store.rs (traits)
        → adapters/fastembed/provider.rs (implementation)
    → service/graph_service.rs (edge add/remove/traversal — a separate
        service; edges have no coupling to the CRUD/embedding flow)
      → ports/storage.rs (trait, for key-or-id resolution)
      → ports/graph.rs (trait)
        → adapters/sqlite/store.rs (implementation)
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

## Engineering Bar

Implement every change as a senior Rust engineer would: idiomatic, minimal,
and production-ready — not merely code that compiles and passes tests. Favor
clarity and simplicity over cleverness. The sections below (Coding
Constraints, Idiomatic Rust Rules, Common Mistakes to Avoid) are the
concrete, checkable expression of this bar — follow them precisely rather
than treating this statement as a substitute for them.

## Coding Constraints

- **No async** — fully synchronous; tokio is not a dependency.
- **No `unwrap` in library code** — use `?` or explicit `expect` with a message.
- **SQL as const** — all SQL strings are `const &str` constants, never inline string literals. Dynamic DDL (e.g. the vec0 dimension) uses a `const` template with `.replace()` at runtime.
- **Binary name is `kb`** — configured via `[[bin]]` in `Cargo.toml`.
- **Error types** — domain errors go in `Error`; startup/config errors go in `AppError`.
- **Storage init** — call `store.initialize()`, `store.initialize_vectors(dims)`, then `store.initialize_graph()` once at startup; all three are idempotent.
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
- **Flatten nested conditions** — prefer method chaining over nested `if` blocks. Instead of `if let Some(x) { if foo(x).is_none() { return Err(...) } }`, write `if let Some(x) { foo(x).ok_or(Error::...)?; }`. Reduce nesting by returning or propagating early.
- **Don't encode structured data into strings** — avoid join→split round-trips (e.g. joining a `Vec<String>` into a comma-separated `String` in the service only to split it again in the handler). Either carry the structured data through the error payload or use the `Display` impl directly for output.
- **Extract private helpers for repeated logic** — if the same block of 3+ lines appears in two or more methods, extract a private helper. Duplication in the service layer is especially likely when the same validation runs on both `add` and `update` paths.
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
- **Duplicating validation logic across methods** — if `add` and `update` share the same guard (e.g. parent existence check), that guard belongs in a single private helper called by both. Copy-pasted validation blocks drift out of sync.
- **Encoding structured data as a delimited string** — joining a `Vec` into a string just so the caller can split it is a design smell. Pass structured data (vec, iterator) through the type, or format it only at the output boundary.
