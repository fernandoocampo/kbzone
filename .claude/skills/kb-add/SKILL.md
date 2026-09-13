---
name: kb-add
description: >
  Adds a new entry to the kbzone knowledge base (SQLite-backed personal KB,
  binary `kb`) via `kb add --json`. Non-interactive and scriptable: takes a
  single JSON object with the entry's fields, validates it, saves it (also
  indexing its embedding), and returns the created entry as JSON. Required
  fields: key, value, category, tags (non-empty array). Use whenever the
  user wants to save/add/remember something to their knowledge base.
argument-hint: '<json-object>'
arguments:
  - entry
allowed-tools:
  - Bash(kb add --json*)
---

Build a single JSON object for the entry from the user's request, then run:

```bash
kb add --json '$ARGUMENTS'
```

## Fields

| Field | Required | Description |
|---|---|---|
| `key` | **yes** | Unique string identifier for the entry (e.g. `rust-ownership`) |
| `value` | **yes** | Main content / answer |
| `category` | **yes** | Entry type (e.g. `concept`, `quote`, `command`, `bookmark`) |
| `tags` | **yes** | Non-empty array of search tags; blank entries are rejected, exact duplicates are silently deduped |
| `reference` | no | Source: author, book title, URL, or person's name |
| `notes` | no | Extended notes or elaboration |
| `namespace` | no | Grouping scope (e.g. `rust`, `k8s`, `personal`) |
| `path` | no | Hierarchical path (e.g. `/personal/rust`); leading `/` is added automatically |
| `parent` | no | UUID of the parent KB entry |
| `media_url` | no | URL or local file path, for media categories |
| `metadata` | no | Freeform key-value object; keys must not be blank |

## JSON Schema

```json
{
  "type": "object",
  "properties": {
    "key": { "type": "string", "minLength": 1 },
    "value": { "type": "string", "minLength": 1 },
    "category": { "type": "string", "minLength": 1 },
    "tags": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "minItems": 1
    },
    "reference": { "type": "string", "default": "" },
    "notes": { "type": "string", "default": "" },
    "namespace": { "type": "string", "default": "" },
    "path": { "type": ["string", "null"], "default": null },
    "parent": { "type": ["string", "null"], "default": null, "description": "UUID of the parent KB entry" },
    "media_url": { "type": ["string", "null"], "default": null },
    "metadata": {
      "type": "object",
      "additionalProperties": { "type": "string" },
      "propertyNames": { "minLength": 1 },
      "default": {}
    }
  },
  "required": ["key", "value", "category", "tags"],
  "additionalProperties": false
}
```

Example:

```bash
kb add --json '{"key":"rust-ownership","value":"Each value has a single owner.","category":"concept","namespace":"rust","tags":["rust","memory"]}'
```

## Output to present to the user

On success, `kb add --json` prints only the created entry as pretty JSON
(`id`, `key`, `value`, `notes`, `category`, `namespace`, `reference`,
`tags`, `metadata`, `path`, `parent`, `created_on`). Confirm the save by
showing the new `id` and `key` back to the user.

On failure, surface the error message verbatim — common ones are:
- `missing required field: key` / `value` / `category`
- `tags must be a non-empty array`
- `metadata keys must not be blank`
