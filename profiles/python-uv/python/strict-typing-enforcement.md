# Strict Typing Enforcement

Never use `Any` or `object` in types. Always model explicit schema types.

```python
# ✓ Good: Explicit types
from pydantic import BaseModel, Field

class TaskRequest(BaseModel):
    input_text: str
    output_format: str
    model: str = Field(..., description="Model identifier")

def process(request: TaskRequest) -> TaskResult:
    """Process with explicit types - ty will catch errors."""
    ...

# ✗ Bad: Any or object
from typing import Any

def process(request: Any) -> Any:
    """Process with Any - no type safety."""
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

class TaskRequest(BaseModel):
    input_text: str = Field(..., min_length=1, description="Input text to process")
    output_format: str = Field(..., pattern=r'^[a-z_]+$', description="Output format identifier")
    model: str = Field(..., description="Model identifier")

# ✗ Bad: Raw type annotation without Field
from pydantic import BaseModel

class TaskRequest(BaseModel):
    input_text: str  # No description, no validators
    output_format: str
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
