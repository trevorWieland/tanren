# No Scenario Skipping

BDD scenarios must run or fail. Skip paths are disallowed.

**Rules:**
- No `it.skip`, `describe.skip`, `test.todo`, or conditional skip logic in CI suites
- No tag-based exclusion used to hide failing behavior scenarios
- Fix flaky scenarios or remove them with explicit behavior deprecation changes

**Why:** Skipping scenarios breaks the behavior contract and invalidates confidence signals.
