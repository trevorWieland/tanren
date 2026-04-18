---
kind: standard
name: file-size-budget
category: rust-style
importance: medium
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- style
---

Split files that grow past 500 lines by responsibility; extract helpers when functions exceed 100 lines.
