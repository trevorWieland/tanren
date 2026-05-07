---
schema: tanren.experience_state_matrix.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-07
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

## Behavior State Exceptions

The default-state coverage above applies to every behavior unless an exception
is recorded here. Exceptions are areas of the behavior catalog where a
non-default state is intrinsic to the behavior intent and `craft-roadmap` must
size additional proof obligations.

| Area | States that need explicit per-behavior design | Notes |
|------|------------------------------------------------|-------|
| `governance` | `permission_denied`, `audit-trail`, `policy_violation` | Every behavior in this area declares an explicit policy boundary; default `permission_denied` copy is insufficient. |
| `configuration` | `permission_denied`, `secret_redaction`, `stale` | Credential and secret behaviors must verify redaction in error and log paths. |
| `runtime-substrate` / `runtime-actor-contract` | `unavailable`, `pending`, `partial` | Substrate failures cascade; behaviors must define partial-progress and resume semantics. |
| `observation` | `stale`, `partial`, `empty` | Read-only projections must distinguish "no data" from "stale projection" and "partial replay". |
| `undo-recovery` | `unavailable`, `permission_denied`, `irreversible` | Some operations are irreversible past a window; that boundary is per-behavior copy. |
| `autonomy-controls` | `permission_denied`, `policy_violation` | Approval-required behaviors must fail closed with explicit operator path. |
| `external-tracker` / `integration-management` / `integration-contract` | `unavailable`, `stale`, `auth-failure` | External-system behaviors must define the local-state-vs-external-state staleness contract. |

## Behavior State Inventory

A complete `behavior-id × state` exception list is generated as part of
`craft-roadmap` shaping (it lives in spec evidence, not here). This file
records the area-level patterns that should always be considered.
