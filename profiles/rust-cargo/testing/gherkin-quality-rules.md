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
- One `Feature` per file, named `B-XXXX-<slug>.feature`
- Scenario titles must describe user-observable outcomes
- Keep steps outcome-focused, not implementation-focused
- Keep active feature files under `tests/bdd/features`
- The closed tag allowlist enforced by `xtask check-bdd-tags`:
  - `@B-XXXX` — feature-level only (one per `.feature` file). The
    behavior ID is NEVER applied at the scenario level.
  - `@positive` or `@falsification` — exactly one per scenario.
  - `@web | @api | @mcp | @cli | @tui` — one or two per scenario, each
    naming an interface the scenario actually drives.
- `Scenario Outline` and `Examples:` blocks are FORBIDDEN. Write each
  variation as its own `Scenario` so the witness is unambiguous.
- Do not use phase, wave, proof, tier, skip, ignore, pending, or WIP tags
  in the required behavior suite.

**Source-of-truth note:** The canonical authority for tag rules is
`docs/architecture/subsystems/behavior-proof.md` § "BDD Tagging And File
Convention". Where this profile and that record disagree, the architecture
record wins.

**Why:** Precise Gherkin lowers ambiguity and prevents scenario drift. A
closed tag allowlist plus per-scenario interface witnesses make the proof
mechanically auditable.
