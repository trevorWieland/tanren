---
kind: standard
name: thiserror-for-libraries
category: rust-error-handling
importance: high
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- error-handling
---

Libraries must return `thiserror`-derived enums; `anyhow` is permitted only in `bin/` crates.
