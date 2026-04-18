---
kind: standard
name: tracing-over-println
category: rust-observability
importance: critical
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- observability
---

Emit via `tracing`; configure a stderr subscriber in binaries. Workspace-level clippy denies `print_stdout`, `print_stderr`, `dbg_macro`.
