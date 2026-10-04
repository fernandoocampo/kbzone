---
name: kb-search
description: >
  Information retrieval across the kbzone knowledge base (SQLite-backed
  personal KB, binary `kb`): semantic natural-language search (`ask`),
  structured keyword/filter search (`search`), listing distinct categories
  (`categories`), and a random entry from any category (`random`). Use whenever
  the user wants to find, search, look up, recall, browse, or discover entries
  — "what do I have on X", "find my notes about Y", "show me everything tagged
  docker", "what categories do I use", "give me a random quote/idiom/concept" —
  not just when they say the word "search". Always requests JSON output
  (`--out json`) and parses it rather than scraping plain text.
allowed-tools:
  - Bash(kb ask *)
  - Bash(kb search *)
  - Bash(kb categories*)
  - Bash(kb random *)
---

## Picking a mode

| User intent | Mode | Command |
|---|---|---|
| Open-ended / conceptual question ("what do I know about X", "how do I do Y") | semantic | `kb ask` |
| Filter by keyword, category, namespace/path/key (support `*` wildcards), tags, or reference | keyword | `kb search` |
| "what categories exist" / "what kinds of things have I saved" | categories | `kb categories` |
| "give me a random quote/idiom/concept" | random | `kb random` |

Default to semantic search (`ask`) for open-ended, natural-language questions; use `search` when the user names specific filters (a category, a tag, a reference). They're complementary, not interchangeable — `ask` ranks by meaning, `search` matches exactly.

## Semantic search — `kb ask`

```bash
kb ask "<natural language query>" --limit 10 --threshold 0.9 --category <cat> --namespace <ns> --out json
```

- `--threshold` (default `0.9`) is a **maximum distance** — lower is stricter. If results feel too sparse, raise it (e.g. `0.95`) rather than assuming there's nothing relevant.
- `--limit` (default `10`).
- `--category` and `--namespace` are both optional and combine with AND semantics when both given. Only pass them when the user actually named a category/namespace to scope to — don't invent filters they didn't ask for, since narrowing an otherwise-open-ended question can hide the answer.
  - `--namespace` scopes the vector search itself (it's an exact-match partition filter — no `*` wildcards here, unlike `search`), so it's cheap and precise — good default when the user says things like "in my rust notes" or names a project/domain.
  - `--category` is applied after the nearest-neighbor search as a filter, so it's still exact-match but slightly less precise at very small `--limit` values — if a `--category` filter returns fewer results than expected, retry with a higher `--limit` before concluding there's nothing relevant.
- Response is a list of `{ "item": {...}, "score": <distance> }`; lower `score` = closer match. `item` here is a lighter DTO (id, key, category, namespace, tags) — follow up with `kb-manage-entry`'s `get` if the user needs the full value/notes.

## Keyword / filter search — `kb search`

```bash
kb search --keyword <kw> --category <cat> --namespace <ns> --path <path> --key <key> --tags <t1,t2> --reference <ref> --start-date <YYYY-MM-DD> --end-date <YYYY-MM-DD> --limit 20 --offset 0 --out json
```

- `--keyword` runs an FTS5 full-text match; the other flags are exact/contains filters and combine with it (AND semantics).
- `--namespace` supports `*` glob wildcards (any number, any position — translated to SQL `%`) and matches case-insensitively whenever a `*` is used, e.g. `--namespace 'company.domain.*'` (prefix), `--namespace '*.domain.subdomain'` (suffix), `--namespace '*.domain.subdomain.*'` (contains). A plain value with no `*` is an exact match. This wildcard support is `search`-specific — it does not apply to `ask`'s `--namespace`.
- `--key` supports `*` glob wildcards on the entry key (same behavior as `--path`: always case-insensitive, `*` at any position), e.g. `--key 'user-*'` (prefix), `--key '*-handler'` (suffix), `--key '*auth*'` (contains). A plain value with no `*` is an exact match. Use it when the user remembers part of an entry's key name.
- `--start-date`/`--end-date` (`YYYY-MM-DD`) filter by the entry's `created_on` creation timestamp, inclusive on both ends. Only pass the ones the user actually asked for (e.g. "created this month", "since March 1st").
- Only pass flags the user actually specified — don't invent filters they didn't ask for.
- `--limit` default `20`, `--offset` default `0` for pagination.
- JSON response has the structure: `{ "results": [...], "total": <count>, "duration_ms": <ms> }`. Each result item uses the lighter DTO shape (id, key, category, namespace, tags) — no value/notes/reference/metadata; use `get` for full detail.
- `total` is the count of **all matching entries** (ignoring limit/offset), useful for pagination awareness.
- `duration_ms` is the query execution time in milliseconds.

## Categories — `kb categories`

```bash
kb categories --namespace <ns> --out json
```

`--namespace` is optional (scopes the list to one namespace). Returns a flat JSON array of distinct category strings.

## Random entry from a category — `kb random`

```bash
kb random --category <category> [--namespace <ns>] [--include-notes] [--out json]
```

- `--category` is required; specifies which category to sample from (e.g. `quote`, `english-idioms`, `concept`).
- `--namespace` is optional; narrows to a specific namespace if given.
- `--include-notes` is optional; when passed, appends the entry's notes field (if present) to the output.
- `--out json` is optional; returns the full entry as JSON. Without it, plain-text output shows value, reference, and (if `--include-notes`) notes.

Unlike the earlier `kb quote` command (which only worked for the `quote` category), this is generic — use it to sample any category you have entries in.

## Output to present

Summarize results as a readable list (key, category, tags, short snippet) unless the user asked for raw JSON. If nothing matched, say so plainly rather than treating an empty array as an error — it's the CLI's normal way of reporting no results.

## Follow-up: Getting full entry details

Search results return a lightweight DTO with only `id`, `key`, `category`, `namespace`, and `tags`. To fetch the complete entry (including `value`, `notes`, `reference`, and `metadata`), use:

```bash
kb get --key <key> --out json
```

Use the `key` field from the search result, not the `id`. To fetch multiple entries, run `kb get --key` once per entry.
