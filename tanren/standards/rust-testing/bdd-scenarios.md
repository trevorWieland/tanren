---
kind: standard
name: bdd-scenarios
category: rust-testing
importance: critical
applies_to:
- '**/*.rs'
- '**/*.feature'
applies_to_languages:
- rust
applies_to_domains:
- testing
- behavior
---

Behavior proof is expressed as executable `.feature` scenarios (cucumber-rs) tagged with a stable behavior ID (`@BEH-*`). Every in-scope behavior needs both a positive witness (scenario demonstrates the behavior) and a falsification witness (scenario demonstrates the inverse is rejected). Unit and integration tests via `nextest` remain in support of the scenario suite — they do not substitute for scenario ownership of a behavior ID.
