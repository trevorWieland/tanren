# Gherkin Quality Rules

Gherkin quality is enforceable and part of the Rust testing standard.

**Rules:**
- One `Feature` per file
- Scenario titles must describe user-observable outcomes
- Use `Scenario Outline` for parameter variation
- Keep steps outcome-focused, not implementation-focused
- Include behavior and tier tags on every scenario

**Why:** Precise Gherkin lowers ambiguity and prevents scenario drift.
