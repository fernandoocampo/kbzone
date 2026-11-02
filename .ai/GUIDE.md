# AI-Method Guide: scars, Makefile, Committed Speech, init.d state

Distilled from 11 dev.to/gde articles (Randal Schwartz's series). Where I adapted or invented something, it is marked **[adapted]** or **[mine]**. Snippets were read through a summarizing fetcher, so verify anything you plan to treat as canonical against the source. The author's metrics (0% repeat failures, first-pass rates) are self-reported by one team.

## 1. Mental model (five ideas)
| Idea | Replaces | Why |
|---|---|---|
| **Makefile DAG** (`templates/Makefile.workflow`) | long numbered prompt checklists | models know `make`; progress is read from files/git/CI, not remembered |
| **Committed Speech** (`templates/handshake.md`) | vague instructions, "do NOT" lists | Request/Promise/Assert/Declare with Counter-Offer and Decline as legal answers stops hallucinated promises |
| **init.d state files** (`templates/state/NN_*.md`) | the context window as memory | survives compaction; the files outrank any summary |
| **Scars** (`templates/scar.md`) | repeating mistakes across sessions | Wound / Trap / Permanent Reflex stored outside the session |
| **Sensors** (`templates/hooks/`, lint, tests) | prose rules | a check that answers from outside the model cannot be argued with. "The best prompt rule is the one you delete." |

Use known formalisms (Makefile, init.d, EBNF, circuit breaker), not a custom prompt DSL: the model has seen them far more often.

## 2. One-time bootstrap per project
```
cp -r <ai-method>/templates  <project>/.ai
mkdir -p <project>/.ai/state/approvals <project>/.ai/scars
```
1. Project `Makefile`: add `include .ai/Makefile.workflow`; set `FORMAT_CMD`, `LINT_CMD`, `TEST_CMD` (Go defaults provided; Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`).
2. Fill `.ai/state/00_governance.md`, then `chmod 0444` it and `.ai/critic-rubric.md`.
3. Copy `scar-index.md` to `.ai/scars/scar-index.md`; list global modules (from `<ai-method>/scars/global/`) and create `.ai/scars/project.md` (empty).
4. `CLAUDE.md` (short on purpose):
   ```
   Use Committed Speech (.ai/handshake.md). Start every session with `make read-init-d`:
   read .ai/state/*.md first; they outrank any summary. Work via `make finish`.
   Load scars listed in .ai/scars/scar-index.md for the files you touch.
   ```
5. Optional: `ln -sf ../../.ai/hooks/pre-commit.sh .git/hooks/pre-commit`.

## 3. Starting a new task: do I write a new Makefile prompt?
**No.** The Makefile is reusable infrastructure. A task is one `!Request`.
1. Write the request using `templates/request.md` (goal, premises, testable Conditions of Satisfaction, By-When). Save as `.ai/state/10_ticket.md`.
2. Open a session, paste the handshake (or rely on CLAUDE.md), and say: `Execute make finish for the request in .ai/state/10_ticket.md.`
3. The agent walks the targets; you only act at gates:

| Target | Agent does | Artifact | Human? |
|---|---|---|---|
| 00 | read state, 4-case switch | (reads all) | |
| 10 | record the request | `10_ticket.md` | you write it |
| 15 | archaeology: verify each premise, load scars | `15_archaeology_receipt.md` | |
| 20 | offer a Promise (plan) | `20_plan_approved.md` | **approve** (`touch .ai/state/approvals/plan.ok`) |
| 30 | failing test, then code (Red→Green) | diff | |
| 40 | fresh critic on the diff | `40_critic_review.md`, `30_critic_summary.md` | |
| 45 | present findings | | **approve** `review.ok` |
| 50 | format/lint/test, tree lock, commit | commit | **approve** `commit.ok` |
| 60–80 | push, CI/bots, land | PR | **approve** `push.ok`, `land.ok` |
| 90 | record scars | scar files | you review the diff |

If a premise is false the legal terminal state is a **Counter-Offer** (even a zero-file plan), not code.

## 4. Writing a good request
- You are the Customer (`!`), the agent is the Performer (`?`).
- Conditions of Satisfaction must be testable by a command. "Keep it clean" is untestable.
- Always give a By-When (a gate or deadline).
- Name premises you believe; the agent must verify them first.
- State invariants positively. Bare "do not X" tends to put X in the model's head.
- Exploring? Ask Socratic questions (`templates/socratic-cheatsheet.md`) instead of dictating the fix.

## 5. State files and resume
| File | Write mode |
|---|---|
| `00_governance.md` | once, then read-only |
| `10_ticket.md`, `15_...`, `20_...` | once (revise 15 only through RETRACTED rows) |
| `30_critic_summary.md` | append-only per review round |
| `40_critic_review.md` | overwritten per round, written by the critic only |
| `99_next_action.md` | overwritten each transition; never while a child runs |

Why split them: one big PROGRESS.md erodes the laws with mutable edits and buries the current step. Facts ("tattoos") are immutable; the pointer ("Polaroid") is mutable.
Cold start after compaction: Target 00 → read files in order → apply the 4-case switch (`OPEN_REQUEST`, `ACTIVE_PROMISE`, `RETRACTED_PREMISE`, `AWAITING_DECLARATION`). If a premise is falsified mid-flight: revoke the promise, cascade dependent facts to UNKNOWN, renegotiate.
`make status` shows where you are.

## 6. Scars: where, how to create, how to improve
**Where.** Three tiers, loaded by path via `scar-index.md` (read only modules matching `git diff --name-only`):
- global agent/process/git scars: `<ai-method>/scars/global/*.md`
- stack scars (Go, Rust): same dir, triggered by `*.go` / `*.rs`
- project scars: `<project>/.ai/scars/*.md`
Keep modules small (each under about 36 KB, manifest about 3 KB).

**Create (Target 90, every task, 5 minutes).**
1. Ask: *What did the critic, CI, a bot, or you catch? What did the agent get wrong? What did you nudge?*
2. If it is a repeatable failure mode, write one scar from `templates/scar.md`:
   - **Wound**: what really happened, concretely.
   - **Trap**: the tempting shortcut (include the snippet).
   - **Permanent Reflex**: a positive, checkable invariant.
   - ID `SCAR-<AREA>-<n>`; add "Applies when" globs so routing works.
3. Add it to `scar-index.md` if it is in a new module. Review the diff yourself.
4. Optional mining **[mine]**: periodically ask an agent to list repeated corrections from your transcripts, then write scars by hand from the list. The author defers transcript mining to a later article I did not read.

**Improve.**
- Merge duplicates; rewrite "don't X" as "X holds before Y".
- Prune when a framework change makes a scar false.
- Edit one scar at a time. **Do not have an LLM rewrite the scar files wholesale**: the series reports bloat (up to about +146%) and "semantic blur".
- Tag provenance (`[article]`, `[mine]`, `[earned]`).

**Lifecycle (article 11: stop storing scars as prose).**
```
markdown scar  ->  compiler/lint diagnostic  ->  AST autofix  ->  tombstone
 (costs tokens)     (no prompt tokens)           (no LLM turn)    (1 line)
```
Promote when a sensor exists AND a negative-witness test proves it fires on the trap AND it passes in CI (the article states exactly this for lint rules). Then replace the block with `templates/tombstone.md`: `Lifecycle: Graduated -> <sensor>`. Reader comments in the article suggest extra states (Candidate/Upstreamed) and a CI check that fails if a tombstone points to a deleted sensor; that is not the author's text, so treat it as **[adapted]** advice.
**Decision test for any rule: "Is this a step (the performer asserts it) or a sensor (answered from outside its reach)?"** Sensors win.
Go/Rust mapping: `go vet`, `golangci-lint` (`errcheck`, `errorlint`, `contextcheck`), `go test -race`, `goleak`; `cargo clippy -D warnings` (`unwrap_used`, `await_holding_lock`), `cargo fmt --check`, `cargo test`. Custom lint rules come later (see roadmap).

## 7. Sensors worth having first
1. **Git tree lock** (`hooks/pre-commit.sh`): what is committed equals what was reviewed.
2. **Read-only tests during Red→Green**: only source changes; record the test tree hash at red and check at green **[adapted]**.
3. **Runner-produced green**: exit code 0 and assertion count above 0, against the real entry point.
4. **Flake policy**: 0 retries locally; one rerun on the same SHA remotely.
5. **Receipts from the measured thing**, not from the message that carries them (hash the diff yourself).

## 8. Review and critic rules
Fresh context, single shot, rubric from disk (`critic-rubric.md`), cites `file:line`, report only. Verdict bound to the diff hash (`git add -N . && git diff HEAD | shasum -a 256`). Write the receipt only after the critic subagent terminated. Re-run after every code change. Different model family for the reviewer if you can. Keep the orchestrator at depth 0: heavy phases run in ephemeral subagents that write a file and return a short summary.

## 9. Failure handling
- 3 edit/test cycles without red→green: stop with `EXIT_THRASH_DETECTED`.
- Circuit breaker: after 2 failures (or 2 bot-driven pushes) stop and report.
- Idempotency for remote actions: check whether the effect already exists before retrying; use `.stamp` files for steps with side effects **[adapted]**.
- Bots: ONE consolidated triage comment, not thread-by-thread.
- No `sleep` in shell; read at most the last ~15 log lines of a child.

## 10. Prompting style
Socratic question over directive; pre-mortem on blast radius ("what else breaks?"); "OK" is a fine answer when the agent defends with evidence; polluted session → extract the invariant, restart the turn.

## 11. Adoption order (smallest useful subset)
1. Handshake + `request.md` (no files needed).
2. `00_governance.md` + `99_next_action.md` (resume after compaction).
3. Scars at Target 90, global + project.
4. Makefile gates + approval files.
5. Critic rubric + tree-lock hook.
6. Graduate the first scar to a lint rule and tombstone it.

## 12. Automation roadmap (not built)
- `ai-init`: scaffold `.ai/` in a project.
- Scar linter: validate headings/IDs/Lifecycle fields; fail if a tombstone's sensor is missing.
- Scar router: print modules for `git diff --name-only`.
- Hook installer; `make -q`-based status; transcript miner for scar candidates.
- Custom lint rules per stack (golangci-lint plugin / clippy-style lints) for graduated scars.

## Source map
1 scars (Wound/Trap/Reflex) · 2 forced continuity, external gates · 3 Socratic prompting, scars as pruning · 4 question nudges, apology spiral · 5 Makefile as workflow · 6 init.d state, compaction, critic integrity · 7 canonical formalisms beat DSLs · 8 Flores handshake · 9 speech acts + Makefile DAG + Belnap · 10 flight tests, Counter-Offer, circuit breaker · 11 sensors over prose, scar lifecycle.
