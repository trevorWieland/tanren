# Log Line Format

All log lines use stable JSONL schema with `{timestamp, level, event, run_id, phase, message, data}` fields.

```python
from pydantic import BaseModel, Field
from datetime import datetime
from typing import Literal

# ✓ Good: Stable Pydantic model for log lines
class LogEntry(BaseModel):
    """Single log line in JSONL format."""
    timestamp: str = Field(..., description="ISO-8601 timestamp")
    level: Literal["debug", "info", "warn", "error"] = Field(
        ..., description="Log level"
    )
    event: str = Field(..., description="Event type (e.g., run_started, phase_completed)")
    run_id: str = Field(..., description="Run identifier")
    phase: str | None = Field(None, description="Execution phase (e.g., process, qa)")
    message: str = Field(..., description="Human-readable log message")
    data: dict | None = Field(None, description="Structured event data")

# ✓ Good: Writing log lines
def write_log(entry: LogEntry) -> None:
    """Write log line to JSONL file."""
    with open("logs/pipeline.log", "a") as f:
        f.write(entry.model_dump_json() + "\n")
```

```json
// ✓ Good: Log line examples
{"timestamp":"2026-01-23T12:00:00Z","level":"info","event":"run_started","run_id":"abc123","phase":null,"message":"Run started","data":{"config_file":"config.toml"}}
{"timestamp":"2026-01-23T12:01:00Z","level":"info","event":"phase_completed","run_id":"abc123","phase":"process","message":"Processing phase completed","data":{"items_processed":1234,"duration_s":45.2}}
{"timestamp":"2026-01-23T12:02:00Z","level":"error","event":"processing_failed","run_id":"abc123","phase":"process","message":"Processing failed for item 42","data":{"item_id":42,"error_code":"RATE_LIMIT","retry_count":3}}
```

**Log line Pydantic model requirements:**
- `timestamp`: ISO-8601 timestamp string (required)
- `level`: One of `debug`, `info`, `warn`, `error` (required)
- `event`: Event name in `snake_case` (required)
- `run_id`: Run identifier (required)
- `phase`: Execution phase name or `None` (optional)
- `message`: Human-readable log message (required)
- `data`: Structured event data as dict or `None` (optional)

**JSONL format:**
- One JSON object per line
- Lines separated by newlines
- No outer array or object wrapper
- UTF-8 encoding

**Event naming conventions:**
- Use `snake_case` for event names
- Examples: `run_started`, `phase_completed`, `processing_finished`, `error_occurred`
- Prefix with phase name when applicable: `process_completed`, `validate_failed`

**Level usage guidelines:**
- `debug`: Detailed diagnostics for development
- `info`: Normal operational messages (phase starts/completions)
- `warn`: Recoverable issues (retries, fallbacks)
- `error`: Failures requiring attention

**Exceptions (external logging systems):**
When integrating with external logging systems (e.g., cloud logging services, ELK stacks), follow their schema requirements but map required fields where possible.

**Why:** Consistent log parsing for observability tools and dashboards; enables easy log analysis and filtering; structured data is machine-readable and queryable.
