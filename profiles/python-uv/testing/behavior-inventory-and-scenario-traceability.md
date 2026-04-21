# Behavior Inventory and Scenario Traceability

Maintain an explicit behavior inventory and require every behavior to map to executable scenarios.

**Rules:**
- Each behavior has a stable ID (for example `BEH-PIPE-001`)
- Each scenario includes `@behavior(<id>)`
- New or modified behavior cannot merge without scenario mapping
- Deprecated behavior must remove or archive its scenarios intentionally
- PRs must include a behavior mapping diff for changed behaviors

**Why:** Behavior IDs make coverage, audits, and demo evidence explicit and reviewable.
