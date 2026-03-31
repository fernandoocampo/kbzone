# kbzone

A personal knowledge base CLI tool with semantic search, built in Rust. Store notes, quotes, commands, and concepts locally — then find them again using natural language.

## What it does

`kbzone` (binary: `kb`) lets you:

- **Add** entries with a key, value, notes, category, namespace, tags, a reference source, and an optional parent entry
- **Search** entries by tag keywords (full-text search via SQLite FTS5)
- **Ask** questions in natural language — finds semantically similar entries using local vector embeddings (no external API calls)
- **List, get, update, delete** entries with flexible filters
- **Export** entries to a YAML file, with optional category/namespace filters and pagination
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
       --reference "The Rust Programming Language" \
       --path /learning/rust
```

The `--path` flag is optional. It accepts Unix-style hierarchical paths like `/personal/rust` or `/work/projects`. The leading `/` is added automatically if omitted — `personal/rust` becomes `/personal/rust`. Invalid paths (e.g. containing `..` or `//`) are rejected with an error message.

To attach an entry to a parent, pass its UUID with `--parent`:

```sh
kb add --key rust-borrowing \
       --value "You can have many immutable references, or one mutable reference — not both." \
       --category concept \
       --namespace rust \
       --parent <parent-uuid>
```

The parent must already exist; the command fails with an error if the ID is not found.

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
kb ask "alfred north whitehead" --threshold 0.85
```

Finds entries by meaning, not just keywords. Returns results ranked by distance (lower = closer match). Use `--threshold` to filter out weakly related results (default: `0.9`).

### Best practices for `kb ask`

`kb ask` performs **semantic / vector search**: your query is converted to an embedding and matched against stored embeddings by cosine similarity — not by exact keywords.

**What gets embedded per entry**

The following fields are combined into the embedding text at index time:

```
{key} {category} {namespace} {reference} {tags} {value[0..200]}
```

To get the best recall:

- **More context = better results** — a full name or phrase (e.g. `"alfred north whitehead"`) produces a much closer match than a single word (`"alfred"`). Short or ambiguous queries yield flat distance distributions where many unrelated entries score similarly.
- **Write descriptive values** — only the first 200 characters are embedded, so lead with the most important information.
- **Fill in `reference`** — author, book title, or person's name. Queries like `kb ask "Chauncey quotes"` will match entries whose reference contains "Chauncey".
- **Use `tags` for domain keywords** — tags are included in the embedding and also power `kb search --keyword`.
- **Set `category` and `namespace`** — these are lightweight signals that help group semantically related entries.
- **Tune `--threshold` to control precision** — the default of `0.9` filters out weakly related results. Lower it (e.g. `--threshold 0.85`) for stricter matching; raise it (e.g. `--threshold 0.95`) if you're getting too few results.

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
| Relevance control | `--threshold` (distance cutoff) | — |
| Best for | "How do I …", "What is …" | Known tag values |

### Update an entry

```sh
kb update --id <uuid> --value "Updated value" --tags rust,ownership,borrow
kb update --id <uuid> --parent <parent-uuid>
kb update --id <uuid> --path /learning/rust
kb update --id <uuid> --path ""   # clears the path
```

Only the fields you pass are changed. Pass `--parent` to set or change the parent; the parent must already exist. Pass `--path` to set or change the path; pass an empty string to clear it.

### Delete an entry

```sh
kb delete --id <uuid>
```

If the entry has children, the delete is rejected and the child IDs are printed. Delete the children first, then retry.

### Import from YAML

```sh
kb import --file my-entries.yaml
kb import --file my-entries.yaml --failed-items-file failures.yaml
```

The YAML file should be a multi-document file (entries separated by `---`). Each document supports the following fields:

```yaml
Key: rust-borrowing
Value: "You can have many immutable references, or one mutable reference — not both."
Notes: ""
Category: concept
Namespace: rust
Tags: [rust, memory, ownership]
Reference: "The Rust Programming Language"
ParentKey: rust-ownership   # optional: kb key of the parent entry
Path: /learning/rust        # optional: Unix-style path; leading / auto-added if omitted
```

`ParentKey` is resolved to an internal UUID at import time. If the referenced key does not exist, that item is recorded as a failure and the rest of the batch continues. Items that fail validation or import are written to the failed items file for inspection.

### Export entries

```sh
kb export --file my-entries.yaml
kb export --file my-entries.yaml --category concept
kb export --file my-entries.yaml --namespace rust
kb export --file my-entries.yaml --category concept --namespace rust
kb export --file my-entries.yaml --limit 100 --offset 0
```

Exports matching entries to a multi-document YAML file in the same format accepted by `kb import`. Filters are cumulative — `--category` and `--namespace` are combined with AND. Use `--limit` and `--offset` for pagination.

Parent–child relationships are preserved: an entry's `Parent` field is only written when its parent is also included in the export set. Parents always appear before their children in the output file so the file can be re-imported directly with `kb import`.

```yaml
Key: motogp-twitter
Value: https://x.com/MotoGP
Notes: First on the throttle, last on the brakes
Category: bookmark
Reference: motogp twitter
Namespace: default
Parent: any-parent-key
Path: /sports/motorsport
Tags:
    - account
    - bookmark
    - motogp
    - motorcycles
    - twitter
```

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
