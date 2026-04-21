# Coverage as Scenario Proxy

Use coverage to find unproven behavior, not to maximize a number.

**Rules:**
- Treat uncovered paths as missing scenario candidates first
- If no behavior is missing, document whether code is dead or supporting-only
- Keep scenario-focused coverage review notes in PRs

**Why:** Coverage is strongest when it drives behavior completeness decisions.
