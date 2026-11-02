# Scar template
<!-- One scar = one H2 block. IDs: SCAR-<AREA>-<n>. AREA examples: PROC, GIT, TEST, GO, RUST, <PROJECT>.
     Write the Reflex as a POSITIVE, testable invariant ("X holds before Y"), never "don't do X". -->

## SCAR-<AREA>-<n>: <short name>
- **Lifecycle:** markdown            <!-- markdown | diagnostic (lint/compiler) | autofix | Graduated -> <where> -->
- **Created:** YYYY-MM-DD  (ticket/PR: <ref>)
- **Applies when:** <path globs or situation, e.g. `**/*.go` touching context / goroutines>
- **Wound:** What really failed, concretely. Where, how it was caught, what it cost.
- **Trap:** The tempting shortcut that leads there (what a model/human naturally writes).
```<lang>
// the trap, minimal snippet
```
- **Permanent Reflex:** The invariant that must hold before code is written or merged.
  Phrase so a reviewer or tool can check it.
- **Sensor (if any):** command or rule that checks the reflex from outside the model, e.g. `golangci-lint` rule, `cargo clippy -D clippy::unwrap_used`, a test name.
- **Promote when:** the sensor exists AND a negative-witness test proves it fires on the trap AND passes in CI.

<!-- Maintenance rules
 - Edit one scar at a time (by hand or a reviewed diff). Never have an LLM rewrite the whole file.
 - Merge duplicates. Rewrite negative phrasing positively. Prune when a framework change makes it false.
 - When Lifecycle becomes Graduated, replace the block with templates/tombstone.md. -->
