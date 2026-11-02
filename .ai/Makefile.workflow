# COMMITMENT-MACHINE WORKFLOW — flat 00..90 DAG (adapted from the articles' "Randal's Preferred Workflow")
# Usage in a project Makefile:   include .ai/Makefile.workflow
# Run:  make finish      (the agent walks the deepest unsatisfied target; recipes below are agent instructions)
#
# Rules:
#  * A target is an artifact or a verified condition, not a step the agent "remembers".
#  * Keep fan-in small (1-3 prerequisites per target).
#  * Human gates are prerequisites whose only rule is "STOP": make halts until a human creates the approval file.
#  * Sensors (exit codes, hashes) decide readiness; the agent does not certify its own work.

STATE ?= .ai/state
APPR  := $(STATE)/approvals

# Stack sensors — override per project (Go shown; Rust: cargo fmt --check / clippy -- -D warnings / test)
FORMAT_CMD  ?= gofmt -l .
LINT_CMD    ?= go vet ./... && golangci-lint run
TEST_CMD    ?= go test -race ./...

.PHONY: finish land ready-to-land bot-quiescence push commit approved-review \
        adversarial-review implementation-diff approved-plan archaeology-verified \
        ticket-assigned read-init-d status

finish: land scar-recorded
	@echo "[Target 90 -> 99_next_action.md] Emit witnessed terminal verification block. COMPLETE."

scar-recorded: land
	@echo "[Target 90] Ask: what did review/CI catch? Write Wound/Trap/Reflex per templates/scar.md; update scar-index."

land: ready-to-land $(APPR)/land.ok
	@echo "[Target 80] gh pr merge, close issue, sync default branch, prune feature branch."

ready-to-land: bot-quiescence
	@echo "[Target 75: ?Declare] Present witnessed assertions and STOP for land approval."

bot-quiescence: push
	@echo "[Target 70 -> 70_bot_triage.md] Poll CI + bots to quiescence; ONE consolidated triage comment; max 2 bot-driven pushes (circuit breaker)."

push: commit $(APPR)/push.ok
	@echo "[Target 60] Verify merge-tree, git push, open PR."

commit: approved-review $(APPR)/commit.ok
	@echo "[Target 50] $(FORMAT_CMD); $(LINT_CMD); $(TEST_CMD); verify tree-hash lock (hooks/pre-commit.sh); git commit."

approved-review: adversarial-review $(APPR)/review.ok
	@echo "[Target 45 -> 45_review_approved.md] Present findings table and STOP for human declaration."

adversarial-review: implementation-diff
	@echo "[Target 40 -> 40_critic_review.md] Spawn FRESH single-shot critic with critic-rubric.md on the diff; bind verdict to diff SHA-256."

implementation-diff: approved-plan
	@echo "[Target 30] Red -> Green. Write failing test first. test/ read-only after red. 3 failed cycles => EXIT_THRASH_DETECTED."

approved-plan: archaeology-verified $(APPR)/plan.ok
	@echo "[Target 20 -> 20_plan_approved.md] Offer !Promise (plan + files + invariants); STOP for human plan approval."

archaeology-verified: ticket-assigned
	@echo "[Target 15 -> 15_archaeology_receipt.md] Read-only: check every premise against code/git log. Belnap ledger. CONTRADICTED => ?Counter-Offer."

ticket-assigned: read-init-d
	@echo "[Target 10 -> 10_ticket.md] Record the !Request: Conditions of Satisfaction + By-When."

read-init-d:
	@echo "[Target 00] Read $(STATE)/*.md in lexical order; evaluate the 4-case switch in 00_governance.md; do NOT trust the compaction summary over these files."

# Human gate: no recipe that can succeed. Creating the file IS the human's declaration.
$(APPR)/%.ok:
	@echo "STOP: awaiting human declaration. Run: mkdir -p $(APPR) && touch $@"; exit 1

status:
	@ls -1 $(STATE) 2>/dev/null; echo "--"; cat $(STATE)/99_next_action.md 2>/dev/null || echo "no 99_next_action.md yet"
