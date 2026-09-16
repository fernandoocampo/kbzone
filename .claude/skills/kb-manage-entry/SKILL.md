---
name: kb-manage-entry
description: >
  CRUD on single entries in the kbzone knowledge base (SQLite-backed personal
  KB, binary `kb`): add, get, update, delete. Use whenever the user wants to
  save/add/remember something new, fetch/look up a specific entry by key or
  ID, edit/correct/update an existing entry's fields, or delete/remove/forget
  an entry — not just when they say "add". Always drives the CLI with its
  structured-output flags (`--json` for add, `--out json` for get/update/
  delete) and returns parsed JSON, never scraped plain text.
allowed-tools:
  - Bash(kb add --json*)
  - Bash(kb get *)
  - Bash(kb update *)
  - Bash(kb delete *)
---

## Picking the action

| User wants to... | Action | Command |
|---|---|---|
| Save / remember / add something new | add | `kb add --json '<entry>'` |
| Look up / fetch / show a specific entry | get | `kb get --key <key>\|--id <id> --out json` |
| Change / edit / correct / rename an entry | update | `kb update --id <id> ... --out json` |
| Remove / delete / forget an entry | delete | `kb delete --id <id> --out json` |

`update` and `delete` are identified by `--id` only (no `--key` flag exists on either) — if the user refers to the entry by key, resolve it first with `get` (see below).

## Add

### Before building the JSON

Ask clarifying questions if any of these are unclear:

- **Category**: What type of entry is this? (e.g. `quote`, `concept`, `command`, `bookmark`, `article`, `idea`)
- **Value vs. notes**: 
  - For `bookmark`: `value` **must be the URL**, `notes` is the description/elaboration
  - For other categories: `value` is the core content — the essential idea, the main command, the key quote, the answer; keep it concise. `notes` is elaboration, context, or background.
- **Namespace** (optional): a topic/domain (e.g. `rust`, `k8s`, `personal`, `work`). Only add if the user explicitly mentions one or it's obvious from context.
- **Reference** (optional): where this came from (author, company, book title, person). For bookmarks, use the author/organization name, not the URL.
- **Tags**: keywords for later retrieval. Extract or infer 3-5 meaningful tags from the content.

### Build and save

```bash
kb add --json '{"key":"rust-ownership","value":"Each value has a single owner.","category":"concept","namespace":"rust","tags":["rust","memory"]}'
```

| Field | Required | Description |
|---|---|---|
| `key` | **yes** | Unique string identifier (e.g. `rust-ownership`) |
| `value` | **yes** | Main content / answer. **For bookmarks: must be the URL** |
| `category` | **yes** | Entry type |
| `tags` | **yes** | Non-empty array; blank entries rejected, exact duplicates deduped |
| `reference` | no | Source: author, company, book title, or person (not URL) |
| `notes` | no | Extended notes or elaboration. **For bookmarks: the description of the resource** |
| `namespace` | no | Grouping scope |
| `path` | no | Hierarchical path (e.g. `/personal/rust`); leading `/` added automatically |
| `parent` | no | UUID of the parent KB entry |
| `media_url` | no | URL or local file path, for media categories |
| `metadata` | no | Freeform key-value object; keys must not be blank |

JSON Schema:

```json
{
  "type": "object",
  "properties": {
    "key": { "type": "string", "minLength": 1 },
    "value": { "type": "string", "minLength": 1 },
    "category": { "type": "string", "minLength": 1 },
    "tags": { "type": "array", "items": { "type": "string", "minLength": 1 }, "minItems": 1 },
    "reference": { "type": "string", "default": "" },
    "notes": { "type": "string", "default": "" },
    "namespace": { "type": "string", "default": "" },
    "path": { "type": ["string", "null"], "default": null },
    "parent": { "type": ["string", "null"], "default": null },
    "media_url": { "type": ["string", "null"], "default": null },
    "metadata": { "type": "object", "additionalProperties": { "type": "string" }, "propertyNames": { "minLength": 1 }, "default": {} }
  },
  "required": ["key", "value", "category", "tags"],
  "additionalProperties": false
}
```

Failure modes to surface verbatim: `missing required field: key/value/category`, `tags must be a non-empty array`, `metadata keys must not be blank`, `InvalidJsonInput`.

## Get

```bash
kb get --key <key> --out json
kb get --id <id> --out json
```

Returns the full entry (`id`, `key`, `value`, `notes`, `category`, `namespace`, `reference`, `tags`, `metadata`, `created_on`, `parent`, `path`, `media_extension`). `--out yaml` is also available; prefer `json` for parsing.

Add `--with-out-connections`, `--with-in-connections`, or `--with-all-connections` when the user also wants to see this entry's relationships in the same call — this saves a separate lookup via the kb-graph skill.

If the key doesn't exist, the CLI prints `Not found.` — report that plainly rather than treating it as a crash.

## Update

Requires the target's `--id`. If the user gave a key instead:

```bash
kb get --key <key> --out json   # resolve id
kb update --id <id> --value "new value" --out json
```

Only pass flags for fields that are actually changing — omitted flags leave that field untouched. `--path ""` clears the path; `--metadata ""` clears all metadata (both replace-in-full, not merge). Ask which fields to change if the user's request is ambiguous about scope (e.g. "fix the note about X" — confirm whether they mean `value` or `notes`).

## Delete

Also identified by `--id`; resolve key → id via `get` first if needed.

```bash
kb delete --id <id> --out json
```

**This is destructive and cannot be undone from the CLI.** Confirm the exact entry with the user before deleting — show them the `get` output first — unless they've already been unambiguous (e.g. they pasted the ID themselves after seeing it).

## Output to present

On success, return the full JSON response as-is — it gives the user the complete, structured entry (or, for delete, the deletion confirmation). On failure, surface the CLI's error message verbatim.
