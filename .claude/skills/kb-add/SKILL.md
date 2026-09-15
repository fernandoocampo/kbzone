---
name: kb-add
description: >
  Adds a new entry to the kbzone knowledge base (SQLite-backed personal KB,
  binary `kb`) via `kb add --json`. Non-interactive and scriptable: takes a
  single JSON object with the entry's fields, validates it, saves it (also
  indexing its embedding), and returns the created entry as JSON. Required
  fields: key, value, category, tags (non-empty array). Use whenever the
  user wants to save/add/remember something to their knowledge base.
argument-hint: '<entry-text>'
arguments:
  - entry
allowed-tools:
  - Bash(kb add --json*)
---

## Before Building the JSON

Before constructing the entry, **ask clarifying questions** if any of these are unclear:

- **Category**: What type of entry is this? (e.g. `quote`, `concept`, `command`, `bookmark`, `article`, `idea`)
- **Value vs. Notes**: Distinguish clearly:
  - `value`: The **core content** — the essential idea, the main command, the key quote, the answer. Keep it concise.
  - `notes`: **Elaboration, context, or background** — additional explanation, where you found it, why it matters, caveats.
- **Namespace** (optional): Is this part of a topic/domain? (e.g. `rust`, `k8s`, `personal`, `work`). Only add if the user explicitly mentions one or if it's obvious from context.
- **Reference** (optional): Where did this come from? (author, book, URL, person's name). Only add if provided.
- **Tags**: What keywords help you find this later? Extract or infer 3-5 meaningful tags from the content.

## Build and Save

Once you have all required fields clearly identified, build a single JSON object and run:

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

On success, return the full JSON response from the CLI as-is. This gives the user the complete entry details in a structured format. The response includes:
- `id` — UUID of the newly created entry
- `key` — the unique key
- `value`, `notes`, `category`, `namespace`, `reference`, `tags`, `metadata`
- `created_on` — ISO-8601 timestamp
- `parent`, `path`, `media_extension` — optional fields

Example output:
```json
{
  "id": "74ec2966-920a-4a16-8607-19568c9e396f",
  "key": "fool-with-a-tool",
  "value": "a fool with a tool is still a fool",
  "notes": "Booch famously and repeatedly reminds the tech industry that engineering judgment, systems thinking, and ethical responsibility cannot be automated away",
  "category": "quote",
  "reference": "Grady Booch",
  "namespace": "",
  "tags": ["quote", "engineering", "ethics", "judgment", "tools", "systems-thinking"],
  "metadata": {},
  "created_on": "2026-09-15T11:29:45+0200",
  "parent": null,
  "path": null,
  "media_extension": null
}
```

On failure, surface the error message verbatim — common ones are:
- `missing required field: key` / `value` / `category`
- `tags must be a non-empty array`
- `metadata keys must not be blank`
- `InvalidJsonInput` — malformed JSON or invalid field values

## When to Ask vs. When to Proceed

- **Ask clarifying questions** if category, value/notes distinction, namespace, or other optional fields are ambiguous
- **Proceed without asking** only when the user's input is complete and clear enough to build all required fields unambiguously
- **Never infer optional fields** (namespace, reference, etc.) without the user explicitly stating them
