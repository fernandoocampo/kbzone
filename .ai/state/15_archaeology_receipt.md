# 15_archaeology_receipt.md — premise ledger (read-only investigation; write once, revise only by RETRACTION)

| # | Premise (from the ticket / my assumptions) | State | Witness |
|---|---|---|---|
| 1 | <e.g. `Foo.Bar()` exists in pkg x> | ESTABLISHED | `x/foo.go:41` |
| 2 | <e.g. bug still present on main> | CONTRADICTED | `git log -S"Bar" -- x/` shows fixed in abc123 |
| 3 | <e.g. API supports pagination> | UNKNOWN | not inspected yet — forbidden to promise on this |

States: UNKNOWN (0,0) | ESTABLISHED (1,0) | CONTRADICTED (1,1) | RETRACTED (0,1)

## Verdict
- [ ] All premises ESTABLISHED -> `?Promise` (go to Target 20)
- [ ] Any CONTRADICTED / suboptimal -> `?Counter-Offer` with modified Conditions of Satisfaction (a zero-file plan is a legal outcome)
- [ ] Goal impossible -> `?Decline`

## Scars consulted
<!-- IDs loaded via scar-index for the files in `git diff --name-only` / the files this ticket will touch -->
