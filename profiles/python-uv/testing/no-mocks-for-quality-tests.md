# No Mocks for Quality Tests

Quality tests use real LLMs (actual model calls). Integration tests must mock LLMs.

```python
# ✓ Good: Quality test with real LLM
# tests/quality/core/processing.py
from myproject.adapters.llm_client import LLMClient

async def test_output_quality_with_real_llm(given_request):
    """Test output quality with actual model call."""
    client = LLMClient(base_url="https://api.openai.com/v1", api_key="test-key")
    result = await client.generate(given_request)

    # Validate actual model output, not mocked response
    assert result.text is not None
    assert len(result.text) > 0

# ✓ Good: Integration test with mocked LLM
# tests/integration/core/processing.py
from unittest.mock import AsyncMock

async def test_processing_flow(given_task_request):
    """Test processing flow with mocked LLM (no real calls)."""
    mock_client = AsyncMock()
    mock_client.process.return_value = TaskResult(text="mocked text", model="gpt-4")

    result = await mock_client.process(given_task_request)
    assert result.text == "mocked text"  # Verify mock, not real model

# ✗ Bad: Quality test with mocked LLM
async def test_output_quality(given_task_request):
    """Quality test must NOT mock LLM."""
    mock_client = AsyncMock()
    mock_client.process.return_value = TaskResult(text="mocked text")
    # This validates mock, not real model behavior - FAILS QA PURPOSE
```

**Quality tests:**
- **REAL LLMs** - actual model calls, not mocked
- BDD-style (Given/When/Then)
- Real services (storage, vector store, file system)
- Minimal mocks (only when unavoidable)
- <30s per test
- Validates actual model behavior and quality, not mocked responses

**Integration tests:**
- **NO LLMs** - mock model adapters for LLM calls
- BDD-style (Given/When/Then)
- Real services (storage, vector store, file system)
- Minimal mocks (only when unavoidable)
- <5s per test
- Tests pipeline flow, not model behavior

**Why real LLMs in quality tests:**
- Validates actual model behavior and quality
- Catches regressions when models change or update
- Ensures prompts and schemas work with real models
- Tests error handling for real API responses

**Why mock LLMs in integration tests:**
- Fast feedback (<5s vs <30s)
- Avoids LLM API rate limits and costs
- Tests pipeline flow, not model behavior
- Makes integration tests deterministic and fast

**Never mock LLMs for:**
- Quality tests (defeats purpose)
- Validating model-specific behavior
- Testing prompt engineering effectiveness
- Validating actual output quality

**Never use real LLMs for:**
- Unit tests (should be <250ms)
- Integration tests (should be <5s)

**API key management for quality tests:**
- Use test API keys with rate limits
- Cache LLM responses when possible (within <30s bound)
- Document API key setup in test docs
- CI should have API key configured for quality tier

**Why:** Validates actual model behavior and quality, not mocked responses; ensures prompts and schemas work with real models and tests error handling for real API responses.
