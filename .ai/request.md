# !Request template  (becomes `.ai/state/10_ticket.md`)

```
!Request: <one sentence: what outcome do I want>

Context / premises I believe (the Performer must verify each BEFORE promising):
- <e.g. package X exposes function Y>
- <e.g. the bug reproduces on main>

Conditions of Satisfaction (each must be testable by a command):
- [ ] `<TEST_CMD>` passes, including a new test that fails before the change
- [ ] `<LINT_CMD>` clean, zero invented APIs
- [ ] Only these paths change: <globs>   (blast radius)

By-When: <deadline or gate, e.g. "plan for approval before any code", "PR ready for review today">

Legal answers: ?Promise | ?Counter-Offer | ?Decline
```

## Phrasing tips
- Testable goal, not "keep it clean". Name the command that proves it.
- Ask for observations, not commitments, when exploring: "What happens to X if Y happens mid-request?" (see socratic-cheatsheet.md)
- Do not list "don't do X" rules; state the invariant that must hold instead.
