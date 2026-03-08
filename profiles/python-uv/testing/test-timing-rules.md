# Test Timing Rules

Enforce strict timing limits per tier. Tests exceeding limits must be rewritten.

```python
# pytest.ini or pyproject.toml
[tool.pytest.ini_options]
markers = [
    "unit: Unit tests (<250ms)",
    "integration: Integration tests (<5s)",
    "quality: Quality tests (<30s)",
]

# CI configuration
# pytest -m unit --durations=0  # Show slowest unit tests
# pytest -m integration --durations=0  # Show slowest integration tests
# pytest -m quality --durations=0  # Show slowest quality tests
```

**Timing limits:**

**Unit tests:** <250ms per test
- Isolated logic and algorithms only
- No external services (network, I/O, databases)
- If test exceeds 250ms, it's not a unit test - move to integration or refactor

**Integration tests:** <5s per test
- Real services (storage, vector store, file system)
- Minimal mocks
- If test exceeds 5s, it's doing too much - split into multiple tests or optimize

**Quality tests:** <30s per test
- Real LLMs (actual model calls)
- Real services (storage, vector store, file system)
- If test exceeds 30s, it's doing too much - split into multiple tests or use cached results

**Enforcement:**

**CI:**
- Fail tests that exceed timing limits
- Use `--durations=0` to flag slow tests
- Block PRs that introduce slow tests

**Locally:**
- Run `pytest --durations=0` to identify slow tests
- Refactor or split slow tests immediately
- Don't ignore timing warnings

**Refactoring strategies:**
- Split large tests into smaller, focused tests
- Cache expensive operations (setup, data loading)
- Remove unnecessary external service calls
- Optimize test data (smaller datasets, fewer iterations)

**Never:**
- Ignore timing warnings or failures
- Comment out slow tests instead of fixing
- Add `@pytest.mark.slow` as loophole (no such marker exists)

**Why:** Fast feedback loop - failing tests identified quickly, prevents test suite from becoming slow and painful to run.
