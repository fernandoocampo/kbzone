# 00_governance.md — constitution + bootloader
<!-- Written ONCE at project start. After creation: `chmod 0444` so a compaction summary or the agent cannot relax it. -->

## Roles
Customer (`!`) = human. Performer (`?`) = agent. Subagents are ephemeral Performers: they write an artifact to disk, return a short summary, and terminate.

## Project
`kbzone` is a synchronous Rust CLI (binary `kb`) over SQLite + sqlite-vec + FastEmbed.
`AGENTS.md` is the source of truth for architecture, style and constraints; read it first (CLAUDE.md mandates this). This file points to it and does not copy it. If the two disagree, AGENTS.md wins and this file is stale.

## Workflow
Active workflow: `.ai/Makefile.workflow` (included by the project Makefile). Run `make finish`.

## Command surface
- Use `make` targets only; `cargo` is never invoked directly (AGENTS.md "Build & Test Commands").
- `FORMAT_CMD` = `make fmt-check`, `LINT_CMD` = `make lint`, `TEST_CMD` = `make test`. Full gate = `make check` (fmt-check + lint + test + vuln-check).
- Run `make fmt` right after every Rust edit, before `make check`.

## Human approval gates (agent STOPS, no tool calls, until the file exists)
- plan -> `.ai/state/approvals/plan.ok`
- commit -> `.../commit.ok`   - push -> `.../push.ok`   - land -> `.../land.ok`   - review -> `.../review.ok`
- A commit happens only after `commit.ok` exists and the user asked for it in the moment. Push and land are human-only.

## Target 00 bootloader — run after ANY wakeup, restart, or context compaction
1. Read `.ai/state/*.md` in lexical order. The files beat any summary in your context.
2. Read the end of the user's request log for a directive newer than `99_next_action.md`; reconcile it before acting.
3. Evaluate the 4-case switch from `99_next_action.md`:
   - `OPEN_REQUEST`          -> run archaeology (Target 15), then offer a plan.
   - `ACTIVE_PROMISE`        -> continue executing tools until the next gate.
   - `RETRACTED_PREMISE`     -> revoke the promise, halt edits, renegotiate (?Counter-Offer).
   - `AWAITING_DECLARATION`  -> halt, `tool_calls: []`, wait for the human.

## Invariants
- Claims cite a witness (file:line, exit code, SHA-256). No witness = "Story".
- The agent never certifies its own work: fresh-context critic, rubric from `.ai/critic-rubric.md`.
- Never write `99_next_action.md` while a background task or subagent is running; yield the turn instead.
- No `sleep` in shell; read only the last ~15 lines of a child's log.
- Scars to load: see `.ai/scars/scar-index.md`.

### kbzone invariants (details in AGENTS.md)
- Plan before code; when a turn goes sideways, stop and re-plan ("AI Workflow Rules").
- Tests are written and seen failing before the logic ("Coding Constraints": TDD).
- Layers depend only downward per the AGENTS.md table; only `application/app.rs` names concrete adapter types ("Hexagonal Architecture").
- Every function takes at most 2 parameters (excluding `self`); a struct carries more.
- Library code uses `?` or `expect("reason")`, never `unwrap()`. Errors keep context: `map_err(|e| Error::Foo(e.to_string()))`. `Error` has no `PartialEq`; tests use `matches!`.
- All SQL is `const &str`. The crate is fully synchronous (no async, no tokio). `domain/` performs no I/O.
- The `key` field on `Kb` stays; graph labels and key/value identity depend on it.
- A change to `Kb::embedding_text()` inputs ships with a note that users must run `kb reindex`.
- Done means `make check` is green (at minimum `make test`), shown by its exit code.
