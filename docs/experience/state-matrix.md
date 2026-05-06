---
schema: tanren.experience_state_matrix.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-05
---

# Experience State Matrix

Each behavior-surface pair should account for the states below before work is
shaped. Not every state needs a separate screen, command, or test, but omitted
states need an explicit reason in the shaped spec or experience contract.

| State | Human GUI | CLI / TUI | Machine / Agent Contract |
|-------|-----------|-----------|--------------------------|
| Entry | Discoverable route, view, or action | Discoverable command, menu, or key path | Discoverable endpoint, schema, or tool |
| Success | Clear completed outcome and next action | Stable output, status line, or exit code | Stable response body or tool result |
| Empty | Useful blank state without false errors | Explicit no-results output | Empty collection or typed no-content result |
| Loading | Progress or pending state where latency is visible | Spinner, progress line, or quiet bounded wait | Retryable pending or accepted status |
| Validation failure | Field or action-level recovery guidance | Input error with correction hint and non-zero exit | Machine-readable validation error |
| Permission denied | Safe denial without leaking hidden resources | Stable denied message and exit code | Stable `permission_denied` code |
| Unavailable | Service or dependency failure with recovery path | Non-zero exit and diagnostic-safe message | Stable `unavailable` or provider failure code |
| Stale | Freshness cue and refresh path | Projection age or refresh instruction | Cursor, version, or stale-projection code |
