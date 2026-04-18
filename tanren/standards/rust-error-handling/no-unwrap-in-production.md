---
kind: standard
name: no-unwrap-in-production
category: rust-error-handling
importance: critical
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- error-handling
---

Library code must not panic. Workspace-level clippy denies `unwrap_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`. Test code may use `expect` for invariants that are themselves tested.
