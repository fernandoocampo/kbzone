---
name: kb-admin
description: >
  System maintenance for the kbzone knowledge base (SQLite-backed personal
  KB, binary `kb`): rebuild search embeddings (`reindex`), export entries
  and their relationships to a YAML backup file (`export`), and check
  build/version info (`version`). Use whenever the user wants to back up or
  export their knowledge base, rebuild/fix semantic search, troubleshoot
  `kb ask` returning stale or missing results, or check which version of
  `kb` is installed.
allowed-tools:
  - Bash(kb reindex*)
  - Bash(kb export *)
  - Bash(kb version*)
---

## Reindex

```bash
kb reindex
```

Rebuilds embeddings for every entry. No flags, no structured output. Reach for this when semantic search (`kb ask`, in the kb-search skill) seems to return stale or missing results — embedding failures on `add`/`update` are non-fatal, so an entry can silently lack an embedding until reindexed.

## Export

```bash
kb export --folder-output <dir> --file-name <name> --category <cat> --namespace <ns> --limit <n> --offset <n>
```

- `--folder-output` is **required** — ask the user where to write the backup if they haven't said.
- `--file-name` optional (defaults to `exported-kb-<yyyy-mm-dd-hh-mi-ss>.yaml`).
- `--category` / `--namespace` scope the export to a subset; `--limit`/`--offset` paginate. Omit any of these the user didn't ask for — a bare `kb export --folder-output <dir>` exports everything.
- Produces one YAML file with `kbs` (entries) and `graph` (edges) sections. There's no `--out` flag — the file itself is the output, and an edge only appears in `graph` if both its endpoints are in the exported `kbs` set (relevant when filtering by category/namespace/limit).
- Report the file path the CLI printed back to the user.

## Version

```bash
kb version
```

No flags, plain text (git hash + build date). Report it as printed.

## Output to present

For `reindex`, confirm completion. For `export`, confirm completion and give the resulting file path. For `version`, relay the printed info directly.
