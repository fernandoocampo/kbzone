---
name: kb-graph
description: >
  Manage and traverse relationships between entries in the kbzone knowledge
  base (SQLite-backed personal KB, binary `kb`): create a link (`link`),
  remove a link (`unlink`), show one-hop relationships (`related`), and walk
  the transitive relationship tree (`tree`). Use whenever the user wants to
  connect/relate two entries, disconnect them, see what an entry is directly
  connected to, or trace a chain of relationships several hops deep — "link
  X to Y", "what's related to X", "show everything connected to X
  transitively", "disconnect X from Y", "how are X and Y related". Requests
  JSON output (`--json` / `--out json`) and parses it rather than scraping
  plain text.
allowed-tools:
  - Bash(kb link *)
  - Bash(kb unlink *)
  - Bash(kb related *)
  - Bash(kb tree *)
---

## Picking an action

| User wants to... | Action | Command |
|---|---|---|
| Connect two entries with a note on why | link | `kb link <from> <to> --note "..." --out json` |
| Remove a connection | unlink | `kb unlink <from> <to>` |
| See what's directly (one hop) connected to an entry | related | `kb related <key-or-id> --json` |
| Trace a multi-hop chain of connections | tree | `kb tree <key-or-id> --json` |

`<from>`/`<to>`/`<key-or-id>` all accept either a `key` or an internal UUID — no need to resolve to an ID first.

## Link

```bash
kb link <from> <to> --note "why they're connected" --out json
```

Edges are **directed**: `A B` and `B A` are distinct edges, and linking the same pair in the same direction twice fails (duplicate edge). `--note` is optional free text. On error the CLI still prints a JSON object like `{"error": "..."}` on stdout — surface it verbatim (common cause: one of `from`/`to` doesn't exist).

## Unlink

```bash
kb unlink <from> <to>
```

No `--out` flag on this command — only plain success/error text. Errors if no such edge exists in that direction.

## Related (one-hop)

```bash
kb related <key-or-id> --direction both --json
```

- `--direction` is `out` (this entry → others), `in` (others → this entry), or `both` (default).
- Note the flag is a bare **`--json`** switch (no value), unlike most other `kb` commands which take `--out json` — don't write `--out json` here, it doesn't exist on this subcommand.
- Response: `{ "node": {...}, "outgoing": [...], "incoming": [...] }`, each edge carrying its `note`.

## Tree (transitive)

```bash
kb tree <key-or-id> --direction out --depth 10 --json
```

- `--direction` is `out` (default) or `in` — there is **no `both`** option for `tree` (unlike `related`).
- `--depth` caps traversal depth, default `10`.
- Same bare `--json` switch as `related`, not `--out json`.
- Response: `{ "root": {...}, "direction": "...", "nodes": [...] }`.

## Note on `kb graph` (interactive view)

There's also a `kb graph <key-or-id>` command that opens an interactive visual graph (vis-network) in the default browser, for drag/click exploration. It's deliberately **not** wrapped here — it produces no JSON, needs internet access to load the viz library, and opens a browser window rather than returning a result. If the user wants a visual/interactive exploration rather than a structured answer, tell them to run `kb graph <key-or-id> [--direction out|in|both] [--depth N]` themselves.

## Output to present

Present relationships readably: from → to, the note, and direction. For `tree`, reflect the hierarchy/depth so the user can see how far each node is from the root. If a result set is empty, say so — it's a normal "no relationships" outcome, not an error.
