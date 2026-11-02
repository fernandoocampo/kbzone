# Scar routing manifest (keep under ~3 KB; mounts scar modules by what changed)
<!-- Load scars by path, not all at once. At Target 15 and Target 40 run:
       git diff --name-only origin/main...HEAD
     then read ONLY the modules whose Trigger matches. Each module stays small enough for one read (< ~36 KB). -->

| Module | Trigger (changed paths / topics) |
|---|---|
| `~/Workspaces/personal/ai-method/scars/global/process.md` | always |
| `~/Workspaces/personal/ai-method/scars/global/git.md` | any commit/push step (Targets 50, 60) |
| `~/Workspaces/personal/ai-method/scars/global/go.md` | `*.go`, `go.mod` |
| `~/Workspaces/personal/ai-method/scars/global/rust.md` | `*.rs`, `Cargo.toml` |
| `.ai/scars/project.md` | always (this project's own scars) |
| `.ai/scars/<area>.md` | <e.g. `internal/billing/**`> |

<!-- Retired scars stay as one-line tombstones in their module; a CI/lint check should fail if a tombstone points to a sensor that no longer exists. -->
