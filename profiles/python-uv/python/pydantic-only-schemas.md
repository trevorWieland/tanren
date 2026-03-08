# Pydantic-Only Schemas

Never use dataclasses or plain classes for schemas. All schemas must use Pydantic.

```python
# ✓ Good: Pydantic schema
from pydantic import BaseModel, Field

class TaskRequest(BaseModel):
    input_text: str = Field(..., min_length=1, description="Input text to process")
    output_format: str = Field(..., pattern=r'^[a-z_]+$', description="Output format identifier")

# ✗ Bad: dataclass
from dataclasses import dataclass

@dataclass
class TaskRequest:
    input_text: str  # No validation, no serialization
    output_format: str

# ✗ Bad: Plain class
class TaskRequest:
    def __init__(self, input_text: str, output_format: str):
        self.input_text = input_text
        self.output_format = output_format
```

**When to use Pydantic schemas:**
- API request/response models
- Configuration models
- Agent input/output schemas
- Storage document models
- Any data that crosses package boundaries

**Pydantic requirements:**
- All fields use `Field(..., description="...")` with clear description
- Use built-in validators for validation (min_length, max_length, pattern, etc.)
- Inherit from `BaseModel` or appropriate Pydantic base
- Type-safe with full type annotations

**Never use dataclasses or plain classes for:**
- Schemas that serialize/deserialize
- Data that requires validation
- Configuration models
- API contracts

**Why:** Pydantic provides automatic validation, native JSON serialization/deserialization, better type inference and IDE support than dataclasses, and catches schema errors early.

**Dataclass migration checklist:**

When migrating `@dataclass` to `BaseModel`:

1. Add `model_config = ConfigDict(extra="forbid")` — dataclasses reject unknown kwargs by default; Pydantic silently drops them without this
2. Preserve `frozen=True` via `ConfigDict(frozen=True)` and `slots=True` via `ConfigDict(slots=True)`
3. Replace all raw field annotations with `Field(..., description="...")`
4. Add built-in validators where constraints are known (min_length, pattern, etc.)
5. Remove `from dataclasses import dataclass` when no longer needed
6. Verify: construct with an unknown kwarg — must raise `ValidationError`
