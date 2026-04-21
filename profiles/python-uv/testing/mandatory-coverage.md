# Coverage from Behavior Scenarios

Coverage is measured from BDD scenario execution and interpreted as a scenario-gap signal.

```bash
# Example: run only scenario suites, then collect coverage
uv run pytest tests/unit/features tests/integration/features tests/quality/features \
  --cov=src --cov-report=term-missing
```

**Rules:**
- Coverage must be generated from scenario-backed test runs
- Uncovered code must be triaged as one of:
  - missing behavior scenario
  - dead/removable code
  - non-scenario support code (explicitly justified)
- PR review must discuss uncovered paths in behavior terms, not only percentage terms
- Coverage thresholds are required, but scenario completeness discussion is primary

**Why:** Coverage becomes evidence about behavior completeness instead of a shallow line-count game.
