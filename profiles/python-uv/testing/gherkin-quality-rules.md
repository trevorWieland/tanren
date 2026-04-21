# Gherkin Quality Rules

Gherkin files must be strict, readable, and machine-checkable.

**Rules:**
- One `Feature` per file
- Scenario titles are outcome-oriented and specific
- Prefer `Scenario Outline` for data variation instead of copy-paste scenarios
- Steps must describe observable behavior, not implementation internals
- Use stable behavior tags and tier tags on every scenario
- Keep Background sections short and reusable

**Why:** High-quality Gherkin keeps behavior specs durable and enforceable over time.
