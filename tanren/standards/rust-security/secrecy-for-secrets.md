---
kind: standard
name: secrecy-for-secrets
category: rust-security
importance: critical
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- security
---

Any field carrying an API key, token, or password must be `Secret<String>` (or a `Secret<CustomType>`); downstream code uses `expose_secret()` at point-of-use and never in a log call.
