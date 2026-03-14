---
name: proceed_without_asking
description: User prefers Claude to proceed with fixes autonomously without asking for confirmation
type: feedback
---

When diagnosing and fixing issues (e.g., diagram syntax errors, rendering problems), just proceed with the fix directly. Do not pause to ask for permission or confirmation before making changes.

**Why:** User explicitly said "don't ask me again, just proceed" during a PlantUML diagram debugging session.

**How to apply:** Whenever a clear fix is identified (even if it requires multiple file edits or iterations), apply it without checking in first.
