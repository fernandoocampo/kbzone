# kbzone

A personal knowledge base CLI tool with semantic search, built in Rust. Store notes, quotes, commands, and concepts locally — then find them again using natural language.

## What it does

`kbzone` (binary: `kb`) lets you:

- **Add** entries with a key, value, notes, category, namespace, tags, a reference source, and an optional parent entry
- **Search** entries by tag keywords (full-text search via SQLite FTS5)
- **Ask** questions in natural language — finds semantically similar entries using local vector embeddings (no external API calls)
- **Organize** entries with hierarchical paths and parent-child relationships
- **Link** entries together with semantic relationships (edges with optional notes)
- **Explore** relationships with one-hop queries, transitive trees, and interactive graph visualization
- **List, get, update, delete** entries with flexible filters
- **Export** entries and their relationships to a YAML file, with optional filters and pagination
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

## Data Model

### KB Item Fields

Each entry in your knowledge base has the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | UUID string | Auto-generated | Unique internal identifier (set automatically on creation) |
| `key` | string | Yes | User-defined identifier; normalized to lowercase on save (e.g. `rust-ownership`, `kubernetes-pods`) |
| `value` | string | Yes | Main content or answer — the core information you're storing |
| `notes` | string | No | Extended notes, elaboration, or additional context |
| `category` | string | No | Entry type for organization (e.g. `quote`, `bookmark`, `concept`, `command`, `media`) |
| `namespace` | string | No | Grouping scope (e.g. `rust`, `kubernetes`, `personal`, `work`) |
| `reference` | string | No | Source attribution — author, book title, URL, person's name. Included in semantic search. |
| `tags` | string[] | No | Comma-separated keywords for full-text search and semantic matching (e.g. `["rust", "memory", "ownership"]`) |
| `metadata` | string[] | No | Comma-separated key=value metadata pairs (e.g. author=me,priority=high) |
| `path` | string | No | Optional Unix-style hierarchical path for filing (e.g. `/learning/rust`, `/work/projects`). Leading `/` is added automatically. |
| `parent` | UUID string | No | UUID of another KB entry to create a hierarchical parent-child relationship |
| `created_on` | ISO-8601 timestamp | Auto-generated | Creation timestamp (set automatically, not editable) |

### Graph Relationships

Beyond hierarchical parent-child links, you can create semantic relationships (edges) between any two KB entries:

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID string | Unique internal identifier for the edge |
| `from_id` | UUID string | Source KB entry ID |
| `to_id` | UUID string | Target KB entry ID |
| `note` | string | Free-text description of why these entries are connected |
| `created_on` | ISO-8601 timestamp | Creation timestamp |

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

### Metadata (custom key-value pairs)

Add arbitrary key-value metadata to an entry using the `--metadata` flag. Pairs are comma-separated, with each pair formatted as `key=value`. Keys must be unique (duplicates are rejected with an error).

```sh
kb add --key rust-ownership \
       --value "Each value has a single owner..." \
       --category concept \
       --namespace rust \
       --tags rust,memory,ownership \
       --metadata "author=me,priority=high,source-format=book"
```

Metadata is stored as JSON in the database and displayed when retrieving an entry:

```sh
kb get --key rust-ownership         # plain text output includes metadata line
kb get --key rust-ownership --out json   # metadata appears as a JSON object
```

Both the interactive prompt (`--interactive`) and `--json` input paths also support metadata.

### Media entries

When `--category media` is used, you must also supply `--media-url` pointing to the file to associate with the entry. The value can be a **URL** (`http://` or `https://`) or a **local file path**.

```sh
# From a local file
kb add --key my-photo \
       --value "A photo from the trip" \
       --category media \
       --namespace personal \
       --media-url /path/to/photo.jpg

# From a URL
kb add --key remote-image \
       --value "Logo downloaded from the web" \
       --category media \
       --namespace assets \
       --media-url https://example.com/logo.png
```

The file is **copied** (not moved) to:

```
{KBZONA_HOME}/media/{namespace}/{key}.{ext}
```

or, if `--path` is also set:

```
{KBZONA_HOME}/media/{namespace}/{path}/{key}.{ext}
```

The original extension is preserved. If the source is a URL, the file is first downloaded to a temporary location and then copied to the final destination.

**Limitations and workflow:**

- `kb update` does **not** support changing `--path` for media entries. If you need to move the file, delete the entry (`kb delete`) and re-create it with the new path.
- `kb delete` on a media entry deletes the media file **first**, then removes the DB record. If the file cannot be deleted, the operation is aborted and the DB record is preserved.
- `kb get` on a media entry shows the computed path to the media file.


### Get an entry

```sh
kb get --key rust-ownership
kb get --id <uuid>
kb get --key rust-ownership --out json
kb get --key rust-ownership --out yaml
```

Retrieve a single entry by key or ID. Use `--out` to format output as `json` or `yaml` (default: plain text).

### Search and filter entries

The `search` command filters, lists, and finds entries by keyword, category, namespace, tags, or reference:

```sh
kb search                                                      # List all entries
kb search --category concept --namespace rust --limit 50      # Filter by category and namespace
kb search --tags rust,memory                                  # Filter by tags
kb search --keyword kubernetes                                # Full-text search in tags (FTS5)
kb search --reference "Kubernetes Official"                   # Filter by reference
kb search --keyword pod --out json                            # Output as JSON
kb search --category concept --limit 10 --offset 20           # Pagination
```

Filters are cumulative — multiple filters are combined with AND logic. Use `--keyword` for full-text tag search (powered by SQLite FTS5). Use `--out` to format results as `json` or `yaml` (default: plain text).

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
kb update --id <uuid> --metadata "priority=high"
kb update --id <uuid> --metadata ""   # clears all metadata
```

Only the fields you pass are changed. Pass `--parent` to set or change the parent; the parent must already exist. Pass `--path` to set or change the path; pass an empty string to clear it. Pass `--metadata` to replace the entire metadata map (comma-separated `key=value` pairs); pass an empty string to clear all metadata. Metadata replaces entirely — to change one key you must retype all keys.

> **Note:** `--path` cannot be changed for entries with `category = media`. Delete and re-create the entry to change the storage path.

### Delete an entry

```sh
kb delete --id <uuid>
```

If the entry has children, the delete is rejected and the child IDs are printed. Delete the children first, then retry.

For `media` category entries, the associated media file is deleted **before** the DB record is removed. If the file cannot be deleted, the operation is aborted so the record is not left without its file.

### Import from YAML

```sh
kb import --file my-entries.yaml
kb import --file my-entries.yaml --failed-items-file failures.yaml --failed-edges-file bad-edges.yaml
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
MediaExtension: jpg         # optional: file extension for media entries (e.g. jpg, png, pdf)
```

`ParentKey` is resolved to an internal UUID at import time. If the referenced key does not exist, that item is recorded as a failure and the rest of the batch continues. Items that fail validation or import are written to the failed items file (default: `wrong-kb-items.yaml`). Relationships (edges) that fail to import are written to the failed edges file (default: `wrong-kb-edges.yaml`).

### Export entries

```sh
kb export --folder-output ./backup
kb export --folder-output ./backup --file-name my-entries.yaml
kb export --folder-output ./backup --category concept
kb export --folder-output ./backup --namespace rust
kb export --folder-output ./backup --category concept --namespace rust
kb export --folder-output ./backup --limit 100 --offset 0
```

Exports matching entries to a multi-document YAML file in the same format accepted by `kb import`. Filters are cumulative — `--category` and `--namespace` are combined with AND. Use `--limit` and `--offset` for pagination. The `--folder-output` directory is required and is where the YAML file will be written; `--file-name` is optional and defaults to `exported-kb-<yyyy-mm-dd-hh-mi-ss>.yaml`.

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

### List all categories

```sh
kb categories
kb categories --namespace rust
kb categories --out json
```

Lists all distinct, non-empty category values in the knowledge base. Use `--namespace` to limit results to a specific namespace. Use `--out json` to print the result as a JSON array; on error, a JSON error object is printed instead.

### Create a relationship link between two entries

```sh
kb link rust-ownership rust-borrowing
kb link <uuid-1> <uuid-2> --note "borrowing is a refinement of ownership"
```

Creates a directed edge (relationship) between two entries. Each argument accepts either a key or a UUID. Use `--note` to add a free-text description of why these entries are related. Attempting to link the same pair twice in the same direction will return an error.

### Remove a relationship link

```sh
kb unlink rust-ownership rust-borrowing
kb unlink <uuid-1> <uuid-2>
```

Removes the directed edge between two entries. Returns an error if no such edge exists.

### Show one-hop relationships

```sh
kb related rust-ownership
kb related rust-ownership --direction out
kb related rust-ownership --direction in
kb related rust-ownership --direction both
kb related rust-ownership --json
```

Shows entries that are directly connected to the given entry (one hop). 

- `--direction out` (default `both`): Show only outgoing edges (entries this one points to)
- `--direction in`: Show only incoming edges (entries that point to this one)
- `--direction both`: Show both directions
- `--json`: Output as JSON with full NOTE text; default plain text truncates NOTE to 40 characters

### Show the relationship tree

```sh
kb tree rust-ownership
kb tree rust-ownership --direction out
kb tree rust-ownership --depth 5
kb tree rust-ownership --json
```

Traverses relationships transitively, showing all connected entries up to a maximum depth.

- `--direction` (`out` | `in`, default `out`): Direction of traversal
- `--depth` (default `10`): Maximum traversal depth
- `--json`: Output as JSON with full NOTE text; default plain text truncates NOTE to 40 characters

### Interactive graph visualization

```sh
kb graph rust-ownership
kb graph rust-ownership --direction both --depth 2
```

Opens an interactive HTML graph view in your default browser showing the entry and its relationships. You can:
- Drag nodes to rearrange the graph
- Click a node to inspect its full content
- See relationships highlighted visually

- `--direction` (`out` | `in` | `both`, default `both`): Relationship directions to display
- `--depth` (default `2`): Maximum traversal depth from the root entry

**Note:** Requires internet access (vis-network loads from CDN). The graph is served once over a loopback HTTP connection; no files are written to disk.

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
