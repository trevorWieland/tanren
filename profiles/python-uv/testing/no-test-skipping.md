# No Scenario Skipping

Scenarios either execute and pass, or execute and fail. Skipping is not allowed.

**Rules:**
- No `@pytest.mark.skip`, no conditional `pytest.skip()` in step code
- No ignored feature files in CI selection
- No "temporary" scenario bypasses
- If a scenario is flaky, fix determinism or remove it
- If behavior is obsolete, delete the scenario and behavior mapping in the same change

**Why:** Skipped scenarios silently corrupt behavior confidence.
