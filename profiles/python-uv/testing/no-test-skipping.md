# No Test Skipping

Never skip tests within a tier. Tests either run and pass or run and fail.

```python
# ✓ Good: Test runs and passes or runs and fails
async def test_translation_with_context(given_scene, given_context):
    """Test runs and validates result."""
    result = await translate_scene(given_scene, given_context)
    
    if result.success:
        assert "character_name" in result.text
    else:
        raise AssertionError(f"Translation failed: {result.error}")  # Test fails

# ✗ Bad: Skip test instead of fixing
import pytest

@pytest.mark.skip(reason="Flaky test, will fix later")  # NEVER DO THIS
async def test_translation_with_context(given_scene, given_context):
    result = await translate_scene(given_scene, given_context)
    assert "character_name" in result.text
```

**Test execution philosophy:**

**Within a tier:**
- Tests either run and pass ✓
- Or run and fail ✗
- Never skip or bypass

**Tier-level execution:**
- Run full unit tier with zero skips
- Run full integration tier with zero skips
- Run full quality tier with zero skips

**No skipping means:**

**No `@pytest.mark.skip`:**
- No "will fix later" skips
- No flaky test skips
- No platform-specific skips (fix test or skip entire tier)
- No configuration skips (make tests work with all configs)

**No conditionals that skip tests:**
- No `if not has_feature: pytest.skip()`
- No `if not on_platform: pytest.skip()`
- No `if not has_credentials: pytest.skip()`

**No CI-level skips:**
- Don't skip test tiers in CI
- Don't skip tests based on conditions
- Run all tiers with `--no-skips` flag

**When tests fail:**

**Fix the test or fix the code:**
- If test is wrong: fix the test
- If code is broken: fix the code
- If test is flaky: make it deterministic or delete it

**Never:**
- Skip failing tests
- Comment out failing tests
- Use `@pytest.mark.xfail` as loophole

**Why tests fail:**

**Flaky tests:**
- Make deterministic (remove randomness, fix race conditions)
- Use explicit setup/teardown
- Delete test if can't be made reliable

**Platform-specific:**
- Fix test to work across platforms or skip entire tier on incompatible platforms
- Document platform-specific behavior in test name, don't skip individual tests

**Configuration-specific:**
- Make tests work with all configurations
- Test multiple configs if needed
- Don't skip based on config state

**CI enforcement:**

**Fail CI on skips:**
- Block PRs that introduce test skips
- Fail entire CI run if any test is skipped
- Use `pytest --no-skips` to ensure no bypass

**CI tier exception:**
- CI skips `make quality` (requires paid API keys). All other tiers run with zero skips.

**Alert on skips:**
- CI should fail loudly if tests are skipped
- Require explicit override to merge with skips (extremely rare)

**Never:**
- Use test skips as bandage for broken tests
- "Will fix later" mentality
- Silent test bypasses

**Why:** Tests either work or fail; no silent bypasses ensures test suite is reliable and trustworthy; prevents accumulation of broken/flaky tests.
