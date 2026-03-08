# BDD for Integration & Quality Tests

Integration and quality tests must use BDD-style (Given/When/Then). Unit tests can use direct assertions.

```python
# ✓ Good: BDD-style for integration/quality tests
from pytest_bdd import given, when, then, scenarios

@given("a configured pipeline with sample script")
def configured_pipeline(tmp_path):
    return setup_pipeline(tmp_path, sample_script)

@when("the pipeline runs to completion")
async def run_pipeline(configured_pipeline):
    configured_pipeline.result = await configured_pipeline.run()

@then("the pipeline produces a playable patch")
def check_output(configured_pipeline):
    assert configured_pipeline.result.success is True
    assert (configured_pipeline.output_dir / "patch.json").exists()

# Integration tests: Use mock model adapter (no real LLMs)
# Quality tests: Use real model adapter (actual LLMs)

# ✗ Bad: Direct assertions in integration/quality tests
def test_pipeline_completion():
    pipeline = setup_pipeline()
    result = await pipeline.run()
    assert result.success is True  # Not BDD-style
```

**BDD structure:**

**Given:** Setup test state
- Arrange test data and fixtures
- Configure system under test
- Document preconditions

**When:** Perform action
- Execute the behavior being tested
- Single action or small sequence
- Document trigger

**Then:** Verify outcome
- Assert expected results
- Validate post-conditions
- Document expected behavior

**Integration tests:**
- BDD-style required
- Real services (storage, vector store, file system)
- Mock model adapters (NO real LLMs)
- <5s per test

**Quality tests:**
- BDD-style required
- Real services (storage, vector store, file system)
- Real model adapters (REAL LLMs)
- <30s per test

**Unit tests:**
- Direct assertions allowed (no BDD requirement)
- Focus on isolated logic
- <250ms per test

**BDD benefits:**
- Tests read like documentation/specs
- Easier to understand for new developers
- Focuses on behavior and scenarios, not implementation
- Clear Given/When/Then structure

**Example scenario:**

```gherkin
Scenario: Pipeline translates scene with context
  Given a configured pipeline with sample script
    And context layer with character definitions
  When the pipeline runs to completion
  Then the pipeline produces a playable patch
    And translations respect character consistency
```

**Never use BDD for:**
- Unit tests (direct assertions are fine)
- Performance benchmarks (different format)
- One-off validation scripts

**Why:** Tests read like documentation/specs (easier to understand) and focuses on behavior and scenarios instead of implementation details.
