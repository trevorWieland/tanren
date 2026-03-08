# Thin Adapter Pattern

Surface layers (CLI, TUI, API, etc.) are **thin adapters only**. All business logic lives in the **Core Domain** packages.

```python
# ✓ Good: CLI is a thin wrapper
def run_pipeline(run_id: str):
    """Start pipeline run - thin adapter over core API."""
    # Calls Core Domain API directly
    result = await pipeline_runner.start_run(run_id)
    return format_json_output(result)

# ✗ Bad: Business logic in CLI
def run_pipeline(run_id: str):
    """Start pipeline run - contains core logic."""
    validate_config(config)  # Business logic
    context = build_context(sources)  # Business logic
    result = translate(context)  # Business logic
    return result
```

**Core Domain logic includes:**
- Pipeline orchestration and phase execution
- Data transformation and validation
- Agent orchestration
- Storage operations
- Model integration

**Surface layers may contain:**
- Command definitions and argument parsing
- Output formatting (pretty-print, JSON wrapper)
- Truly surface-specific features that will never be reused

**Even validation and formatting should use contract models from the Core Domain** - never duplicate schemas or business rules.

**Why:** Ensures core logic is reusable across any surface (CLI, TUI, API, Lambda) without duplication and makes testing easier (test core once, surfaces are just IO wrappers).
