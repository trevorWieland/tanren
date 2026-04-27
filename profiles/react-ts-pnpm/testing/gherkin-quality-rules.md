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
  - typescript
applies_to_domains:
  - testing
---

# Gherkin Quality Rules

Gherkin quality is enforceable and required.

**Rules:**
- One feature per file
- Clear, user-observable scenario titles
- Use scenario outlines for data variation
- Keep steps declarative and behavior-oriented
- Require behavior and tier tags on each scenario

**Why:** Clean Gherkin is easier to maintain, review, and automate.
