# Naming Conventions

Use consistent naming conventions across all code. Never mix styles.

```python
# ✓ Good: Consistent snake_case for modules/functions/variables
from task_engine import process_item
from config_loader import load_config

def handle_item(item: Item) -> TaskResult:
    """Process single item."""
    result = process_item(item)
    return result

# ✓ Good: PascalCase for classes/types
class TaskRequest(BaseModel):
    """Task request model."""

class VectorStoreProtocol(Protocol):
    """Vector store interface protocol."""

class SQLiteIndex:
    """SQLite run metadata index."""

# ✗ Bad: Inconsistent naming
from TaskEngine import process_Item  # PascalCase for module
from config_loader import LoadConfig  # PascalCase for function

def ProcessItem(item: Item) -> TaskResult:  # PascalCase for function
    ...

class taskRequest:  # snake_case for class
    pass
```

**Python code naming:**
- Modules/files: `snake_case.py`
- Functions/variables: `snake_case`
- Classes/types: `PascalCase`

**Database naming:**
- Tables/collections: `snake_case`
- Columns/fields: `snake_case`
- Foreign keys: `{entity}_id` (e.g., `run_id`, `item_id`)

**API naming:**
- CLI commands: `kebab-case` (e.g., `run-pipeline`, `show-status`)
- CLI options: `--snake-case` (e.g., `--run-id`, `--config-file`)

**JSON/JSONL naming:**
- Fields: `snake_case` (e.g., `run_id`, `phase_name`, `error_message`)

**Event naming:**
- Event names: `snake_case` (e.g., `run_started`, `phase_completed`, `processing_finished`)
- Log event names (JSONL `event`) must be `snake_case`
- When phase-specific, prefix with phase name (e.g., `process_completed`, `validate_failed`)

**Why:** Consistency makes code predictable and easier to navigate; reduces confusion when multiple developers/agents work together.
