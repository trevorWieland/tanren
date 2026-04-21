# Behavior Inventory and Scenario Traceability

Maintain explicit behavior IDs and enforce scenario traceability.

**Rules:**
- Every shipped behavior has a stable ID (`BEH-...`)
- Every scenario includes exactly one behavior ID tag
- Behavior-changing PRs must add/update/remove matching scenarios
- Scenario IDs and behavior IDs must remain stable across refactors

**Why:** Stable identifiers make behavior audits, demos, and mutation analysis actionable.
