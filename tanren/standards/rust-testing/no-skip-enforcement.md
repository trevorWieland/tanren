---
kind: standard
name: no-skip-enforcement
category: rust-testing
importance: critical
applies_to:
- '**/*.rs'
- '**/*.feature'
applies_to_languages:
- rust
applies_to_domains:
- testing
- discipline
---

Behavior scenarios may not be skipped, ignored, or conditionally disabled in the cutover suite. `#[ignore]`, `#[cfg(...)]`-gated behavior tests, cucumber `@wip`/`@ignore` tags that bypass CI, and `return Ok(())`-style early exits inside step functions are all prohibited on behavior-owning tests. A failing scenario either gets fixed, its owning behavior gets formally abandoned with an orchestrator event, or the blocker is escalated — it is not hidden. Skip suppression is enforced by the gate tooling, not by convention.
