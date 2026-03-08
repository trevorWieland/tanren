# API Response Format

All CLI JSON responses use `{data, error, meta}` envelope structure. Each field is a clearly defined Pydantic model.

```python
from pydantic import BaseModel, Field

# ✓ Good: Clear Pydantic models for envelope fields
class MetaInfo(BaseModel):
    """Metadata for API responses."""
    timestamp: str = Field(..., description="ISO-8601 timestamp")

class ErrorDetails(BaseModel):
    """Detailed error context."""
    field: str | None = Field(None, description="Field name if validation error")
    provided: str | None = Field(None, description="Value that was provided")
    valid_options: list[str] | None = Field(None, description="Valid values if applicable")

class ErrorResponse(BaseModel):
    """Error information in response."""
    code: str = Field(..., description="Error code (e.g., VAL_001)")
    message: str = Field(..., description="Human-readable error message")
    details: ErrorDetails | None = Field(None, description="Additional error context")

class APIResponse[T](BaseModel, Generic[T]):
    """Generic API response envelope."""
    data: T | None = Field(None, description="Success payload, null on error")
    error: ErrorResponse | None = Field(None, description="Error information, null on success")
    meta: MetaInfo = Field(..., description="Response metadata")
```

```json
// ✓ Good: Success response with Pydantic-validated types
{
  "data": {
    "run_id": "abc123",
    "status": "running",
    "phase": "translate"
  },
  "error": null,
  "meta": {
    "timestamp": "2026-01-23T12:00:00Z"
  }
}

// ✓ Good: Error response with Pydantic-validated types
{
  "data": null,
  "error": {
    "code": "VAL_001",
    "message": "Invalid configuration: model not found",
    "details": {
      "field": "model",
      "provided": "gpt-5.2",
      "valid_options": ["gpt-5-nano", "gpt-5.2", "claude-4.5-sonnet"]
    }
  },
  "meta": {
    "timestamp": "2026-01-23T12:00:00Z"
  }
}

// ✗ Bad: Raw data without envelope
{
  "run_id": "abc123",
  "status": "running"
}

// ✗ Bad: Error without code or structure
{
  "error": "Something went wrong"
}
```

**Response envelope Pydantic model:**
- `data[T]`: Generic type containing success payload, null on error
- `error`: `ErrorResponse` model with `{code, message, details}`, null on success
- `meta`: `MetaInfo` or similar for audit metadata

**Error Pydantic model requirements:**
- `code`: Error identifier (e.g., `VAL_001`, `AUTH_001`, `DB_001`) as required string
- `message`: Human-readable error description as required string
- `details`: Optional `ErrorDetails` or similar for additional context

**Meta Pydantic model requirements:**
- Include timestamp for auditability
- Add version, request_id, or other metadata as needed

**Success response requirements:**
- Always include `data` field with typed payload
- `error` field must be `None` (never omitted in Pydantic model)
- Include `meta` with timestamp for auditability

**Error response requirements:**
- Always include both `code` and `message` in error model
- Include `details` when helpful (field names, valid options, suggestions)
- `data` field must be `None` (never omitted in Pydantic model)

**Exceptions (streaming/event-driven):**
Streaming responses and event-driven outputs (e.g., JSONL logs, SSE events) may use different structures as they don't fit single-response envelope.

**Why:** Frontend always knows where to find data or errors, consistent parsing without guessing schema per endpoint, predictable error handling, and type safety through Pydantic validation.
