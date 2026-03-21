# kbzone

A personal knowledge base CLI tool with semantic search, built in Rust. Store notes, quotes, commands, and concepts locally — then find them again using natural language.

## What it does

`kbzone` (binary: `kb`) lets you:

- **Add** entries with a key, value, notes, category, namespace, tags, and a reference source
- **Search** entries by tag keywords (full-text search via SQLite FTS5)
- **Ask** questions in natural language — finds semantically similar entries using local vector embeddings (no external API calls)
- **List, get, update, delete** entries with flexible filters
- **Import** entries in bulk from a YAML file
- **Reindex** — rebuild embeddings for all entries at any time
- **Quote** — print a random entry from the `quote` category

All data is stored locally in a SQLite file. Embeddings are generated on-device using [FastEmbed](https://github.com/Anush008/fastembed-rs) (BAAI/bge-small-en-v1.5, 384 dimensions). No internet connection is required.

## Prerequisites

- Rust toolchain (1.78+): [rustup.rs](https://rustup.rs)
- `sqlite-vec` is bundled automatically via the crate — no manual installation needed

## Installation

Build the release binary and install it to `~/.cargo/bin`:

```sh
make install
```

Or build only (output goes to `bin/kb`):

```sh
make build
```

## Configuration

On first run, `kb` creates a default config file at `~/.kbzona/config.yaml`:

```yaml
db_path: ~/.kbzona/kbzona.db
embedding:
  provider: fastembed
```

**Override the base directory** with the `KBZONA_HOME` environment variable:

```sh
export KBZONA_HOME=/path/to/custom/dir
```

The embedding model cache is stored alongside the database at `{db_path_parent}/fastembed_cache/`.

## Usage

### Add an entry

```sh
kb add --key rust-ownership \
       --value "Each value has a single owner; when the owner goes out of scope, the value is dropped." \
       --category concept \
       --namespace rust \
       --tags rust,memory,ownership \
       --reference "The Rust Programming Language"
```

### Get an entry

```sh
kb get --key rust-ownership
kb get --id <uuid>
```

### List entries

```sh
kb list
kb list --category concept --namespace rust --limit 50
kb list --tags rust,memory
```

### Full-text tag search

```sh
kb search --keyword kubernetes
```

### Semantic / natural language search

```sh
kb ask "how do I manage memory in Rust"
kb ask "kubernetes pod commands" --limit 5
```

Finds entries by meaning, not just keywords. Returns results ranked by similarity score (lower = closer match).

### Best practices for `kb ask`

`kb ask` performs **semantic / vector search**: your query is converted to an embedding and matched against stored embeddings by cosine similarity — not by exact keywords.

**What gets embedded per entry**

The following fields are combined into the embedding text at index time:

```
{key} {category} {namespace} {reference} {tags} {value[0..200]}
```

To get the best recall:

- **Write descriptive values** — only the first 200 characters are embedded, so lead with the most important information.
- **Fill in `reference`** — author, book title, or person's name. Queries like `kb ask "Chauncey quotes"` will match entries whose reference contains "Chauncey".
- **Use `tags` for domain keywords** — tags are included in the embedding and also power `kb search --keyword`.
- **Set `category` and `namespace`** — these are lightweight signals that help group semantically related entries.

**When to re-run `kb reindex`**

- After a bulk `kb import`
- After upgrading `kbzone` when the embedding text formula changes (e.g. this release adds `reference`)
- If embedding generation failed silently during `add`/`update` (check for missing results in `kb ask`)

**`kb ask` vs `kb search`**

| | `kb ask` | `kb search --keyword` |
|---|---|---|
| Match type | Semantic / conceptual | Exact keyword in tags (FTS5) |
| Query style | Natural language | Single term or prefix |
| Reference filter | Via query text | `--reference <string>` |
| Best for | "How do I …", "What is …" | Known tag values |

### Update an entry

```sh
kb update --id <uuid> --value "Updated value" --tags rust,ownership,borrow
```

Only the fields you pass are changed.

### Delete an entry

```sh
kb delete --id <uuid>
```

### Import from YAML

```sh
kb import --file my-entries.yaml
kb import --file my-entries.yaml --failed-items-file failures.yaml
```

The YAML file should be a multi-document file (entries separated by `---`). Items that fail validation are written to the failed items file for inspection.

### Rebuild embeddings

```sh
kb reindex
```

Use this after an import or if embedding generation failed during `add`/`update`. Embedding failures are non-fatal — entries are always saved; only the vector index may be missing.

### Print a random quote

```sh
kb quote
```

Returns a random entry with `category = quote`.

### Version info

```sh
kb version
```

## Building

```sh
make build      # compile release binary → bin/kb
make install    # install to ~/.cargo/bin/kb
make clean      # remove build artifacts
```

## Testing

```sh
make test       # run all unit and integration tests
make check      # fmt-check + lint + test (full CI gate)
```

Integration tests use an in-memory SQLite database with the sqlite-vec extension loaded, so they run without any external dependencies.

## Linting and Formatting

```sh
make fmt        # auto-format source (run after any code change)
make fmt-check  # check formatting without modifying files
make lint       # run clippy (deny warnings)
```

## Architecture

`kbzone` follows a **hexagonal (ports and adapters) architecture**:

```
CLI args
  → cli/commands.rs (parse)
  → cli/handlers.rs (orchestrate, I/O)
    → service/kb_service.rs (business logic)
      → ports/storage.rs (trait)
        → adapters/sqlite/store.rs (SQLite implementation)
    → service/semantic_service.rs (embedding coordination)
      → ports/embedding.rs + ports/vector_store.rs (traits)
        → adapters/fastembed/provider.rs (local embeddings)
```

| Layer | Location | Role |
|-------|----------|------|
| Domain | `src/domain/` | Pure data types — no I/O, no dependencies |
| Ports | `src/ports/` | Outbound traits (`KbStore`, `VectorStore`, `EmbeddingProvider`) |
| Service | `src/service/` | Business logic — depends only on traits |
| Adapters | `src/adapters/` | Concrete implementations (SQLite, FastEmbed) |
| CLI | `src/cli/` | Command parsing and output formatting |
| Application | `src/application/` | Composition root — wires all layers together |

Diagrams are in [`docs/diagrams/`](docs/diagrams/):
- `01_high_level_architecture.puml` — overall hexagonal architecture
- `02_internal_architecture.puml` — internal component map
- `03_data_model.puml` — domain data model
- `04_ask_sequence.puml` — sequence diagram for `kb ask`
- `05_add_sequence.puml` — sequence diagram for `kb add`

## Project structure

```
src/
  main.rs                        Entry point
  lib.rs                         Module declarations
  errors/error.rs                Error + AppError (thiserror)
  domain/kb.rs                   Kb, NewKb, KbFilter, KbItem, ScoredKbItem, SemanticQuery, EmbeddingInput
  ports/storage.rs               KbStore trait
  ports/embedding.rs             EmbeddingProvider trait
  ports/vector_store.rs          VectorStore trait
  service/kb_service.rs          Business logic + unit tests
  adapters/sqlite/store.rs       SqliteStore (implements KbStore + VectorStore) + integration tests
  adapters/fastembed/provider.rs FastEmbedProvider (BAAI/bge-small-en-v1.5, 384 dims)
  cli/commands.rs                Clap subcommands
  cli/handlers.rs                Command handlers + output formatting
  application/config.rs          Config loading (YAML)
  application/app.rs             App::build() wires deps; App::run() dispatches CLI
```
