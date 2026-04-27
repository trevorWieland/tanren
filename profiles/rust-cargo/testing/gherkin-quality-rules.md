---
kind: standard
name: gherkin-quality-rules
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

# Gherkin Quality Rules

Gherkin quality is enforceable and part of the Rust testing standard.

**Rules:**
- One `Feature` per file
- Scenario titles must describe user-observable outcomes
- Use `Scenario Outline` for parameter variation
- Keep steps outcome-focused, not implementation-focused
- Keep active feature files under `tests/bdd/features`
- Include exactly one `@B-XXXX` tag and one `@positive` or `@falsification`
  tag on every behavior-owning scenario
- Do not use phase, wave, proof, skip, ignore, pending, or WIP tags in the
  required behavior suite

**Why:** Precise Gherkin lowers ambiguity and prevents scenario drift.
