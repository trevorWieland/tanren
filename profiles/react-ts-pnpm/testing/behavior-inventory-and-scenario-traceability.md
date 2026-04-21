# Behavior Inventory and Scenario Traceability

Behavior IDs are mandatory and must map directly to executable scenarios.

**Rules:**
- Each behavior has a stable ID (`BEH-...`)
- Each scenario includes one behavior ID tag
- Behavior changes must include scenario updates in the same PR
- Scenario mapping is required evidence in review

**Why:** Traceability keeps "what the product does" testable and reviewable.
