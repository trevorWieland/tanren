---
kind: standard
name: coverage-as-scenario-proxy
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

# Coverage as Scenario Proxy

Use coverage to find unproven behavior, not to maximize a number.

**Rules:**
- Treat uncovered paths as missing scenario candidates first
- If no behavior is missing, document whether code is dead or supporting-only
- Keep scenario-focused coverage review notes in PRs

**Why:** Coverage is strongest when it drives behavior completeness decisions.
