---
kind: standard
name: behavior-inventory-and-scenario-traceability
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - typescript
applies_to_domains:
  - testing
---

# Behavior Inventory and Scenario Traceability

Behavior IDs are mandatory and must map directly to executable scenarios.

**Rules:**
- Each behavior has a stable ID (`BEH-...`)
- Each scenario includes one behavior ID tag
- Behavior changes must include scenario updates in the same PR
- Scenario mapping is required evidence in review

**Why:** Traceability keeps "what the product does" testable and reviewable.
