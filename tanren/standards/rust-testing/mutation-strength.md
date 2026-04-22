---
kind: standard
name: mutation-strength
category: rust-testing
importance: high
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- testing
- mutation
---

Test suites are gated on mutation strength via `cargo-mutants`. Surviving mutants are treated as evidence of weak scenarios: each survivor must be triaged and resolved against a behavior ID (`BEH-*`) — either by tightening an existing scenario, adding a falsification scenario, or explicitly recording the mutant as equivalent with a linked justification. Mutation regressions fail the final gate; the staged `just` recipes surface mutant output in a machine-consumable form for triage.
