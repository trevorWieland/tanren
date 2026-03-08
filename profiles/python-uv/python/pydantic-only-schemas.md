# Pydantic-Only Schemas

Never use dataclasses or plain classes for schemas. All schemas must use Pydantic.

```python
# ✓ Good: Pydantic schema
from pydantic import BaseModel, Field

class TranslationRequest(BaseModel):
    source_text: str = Field(..., min_length=1, description="Text to translate")
    target_language: str = Field(..., pattern=r'^[a-z]{2}$', description="ISO 639-1 language code")

# ✗ Bad: dataclass
from dataclasses import dataclass

@dataclass
class TranslationRequest:
    source_text: str  # No validation, no serialization
    target_language: str

# ✗ Bad: Plain class
class TranslationRequest:
    def __init__(self, source_text: str, target_language: str):
        self.source_text = source_text
        self.target_language = target_language
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
