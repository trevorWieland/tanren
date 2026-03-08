# Strict Typing Enforcement

Never use `Any` or `object` in types. Always model explicit schema types.

```python
# ✓ Good: Explicit types
from pydantic import BaseModel, Field

class TranslationRequest(BaseModel):
    source_text: str
    target_language: str
    model: str = Field(..., description="Model identifier for translation")

def translate(request: TranslationRequest) -> TranslationResult:
    """Translate with explicit types - ty will catch errors."""
    ...

# ✗ Bad: Any or object
from typing import Any

def translate(request: Any) -> Any:
    """Translate with Any - no type safety."""
    ...
```

**Type configuration:**
- `ty` must be configured in **strict mode**
- Type checking must pass with zero errors before merging
- All schemas, agents, tools, and APIs use explicit types

**Pydantic fields:**
- Every Pydantic field must use `Field(..., description="...")` with a clear description
- Use built-in validators (`min_length`, `max_length`, `pattern`, `gt`, `ge`, `lt`, `le`, etc.) for validation
- Never use raw type annotation without Field for schema fields

```python
# ✓ Good: Field with description and validators
from pydantic import BaseModel, Field

class TranslationRequest(BaseModel):
    source_text: str = Field(..., min_length=1, description="Text to translate")
    target_language: str = Field(..., pattern=r'^[a-z]{2}$', description="ISO 639-1 language code")
    model: str = Field(..., description="Model identifier for translation")

# ✗ Bad: Raw type annotation without Field
from pydantic import BaseModel

class TranslationRequest(BaseModel):
    source_text: str  # No description, no validators
    target_language: str
    model: str
```

**Exceptions (extremely rare, high bar):**
`Any` or `object` only when **all** are true:
1. Fully external library/API
2. Creating types isn't practical
3. No better alternative library or API exists
4. We actually need the feature and cannot avoid it

**Never** use `Any` for internal code, schema definitions, or where types are available.

**Why:** Catches type errors at dev time instead of runtime, makes code more readable and self-documenting, and prevents whole classes of bugs before they happen.
