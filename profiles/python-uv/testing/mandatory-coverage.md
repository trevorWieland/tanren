# Mandatory Coverage

Coverage is mandatory for features. Tests must directly exercise intended behavior.

```python
# ✓ Good: Tests exercise actual behavior
# tests/unit/core/processor.py
async def test_processes_item_with_context(given_item, given_context):
    """Test that processing applies context to output."""
    result = await process_item(given_item, given_context)

    # Directly exercises processing behavior
    assert "character_name" in result.text
    assert result.context_used == given_context.id

# ✗ Bad: Tests don't exercise behavior
async def test_processor_initialization():
    """Just tests constructor, not behavior - NO COVERAGE VALUE."""
    processor = Processor()
    assert processor is not None  # No behavior tested
```

**Coverage requirements:**

**For every feature:**
- Tests must directly exercise intended behavior
- All code paths must be covered (happy path, error cases, edge cases)
- No uncovered production code
- Coverage must pass in CI before merging

**CI enforcement:**
- Run `pytest --cov=src/`
- Fail PRs below coverage threshold (e.g., 80%)
- Block merging new features without tests
- Generate coverage reports for review

**What counts as coverage:**

**✓ Direct behavior testing:**
- Unit tests calling functions with real inputs
- Integration tests running full workflows
- Quality tests validating end-to-end behavior
- Error path and edge case testing

**✗ NOT coverage:**
- Constructor-only tests (no behavior)
- Property access tests (no behavior)
- Mock-only tests (no real logic)
- Tests that bypass or stub out production code

**Coverage strategies:**

**Unit tests:**
- Test isolated functions and methods
- Mock external dependencies
- Cover all branches and error paths
- Use parametrization for edge cases

**Integration tests:**
- Test workflows and component interaction
- Use real services (storage, vector store)
- Cover cross-component code paths
- Validate end-to-end behavior

**Quality tests:**
- Test with real LLMs
- Validate actual outputs, not mocks
- Cover model integration paths
- Ensure prompt/response handling

**Never merge:**
- New features without tests
- Tests that don't exercise behavior
- Uncovered code paths
- Failing coverage checks in CI

**Why:** Catches regressions early by testing actual behavior; ensures code quality and test hygiene by preventing untested production code.
