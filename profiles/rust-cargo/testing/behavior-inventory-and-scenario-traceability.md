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
  - rust
applies_to_domains:
  - testing
---

# Behavior Inventory and Scenario Traceability

`docs/behaviors` is the behavior source of truth. Executable scenarios prove
accepted behavior docs through stable `B-XXXX` IDs.

**Rules:**
- Every accepted behavior has a stable `B-XXXX` ID.
- Every behavior-owning scenario includes exactly one `@B-XXXX` tag.
- Every behavior-owning scenario includes exactly one witness tag:
  `@positive` or `@falsification`.
- Every accepted behavior has at least one passing positive witness and one
  passing falsification witness.
- Draft behaviors may exist without executable scenarios.
- Deprecated behavior IDs cannot appear in active scenarios unless the
  scenario is explicit compatibility, migration, or deprecation coverage.
- Traceability is generated from behavior docs, feature tags, execution,
  coverage, and mutation artifacts; hand-maintained traceability JSON is not a
  source of truth.

**Why:** Product behavior IDs need to survive refactors, roadmap phase changes,
and test-suite reshaping without drifting away from executable evidence.
