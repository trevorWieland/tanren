---
kind: standard
name: behavior-traceability
category: rust-testing
importance: critical
applies_to:
- '**/*.rs'
- '**/*.feature'
- 'tanren/specs/**/*.md'
applies_to_languages:
- rust
applies_to_domains:
- testing
- traceability
---

Every testable behavior has a stable `BEH-*` identifier that is referenced from: (1) the spec/plan behavior inventory, (2) the cucumber scenario via a `@BEH-*` tag, (3) mutation survivor triage records, and (4) coverage classification records. Behavior IDs are immutable once published; renames require an abandon+replace event pair in the orchestrator. A behavior claim without a resolvable `BEH-*` link in all four artifacts is treated as incomplete traceability and fails the final gate.
