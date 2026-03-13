# Spec Lifecycle

Tanren is intentionally opinionated about this lifecycle.

## Core Lifecycle

`draft -> shaped -> executing -> validating -> review -> merged`

## Ten Workflow Responsibilities

1. Issue intake and backlog curation
2. Shape spec
3. Orchestrate execution
4. Walk spec
5. Gate check
6. Handle feedback
7. Merge conflict resolution
8. Dependency management
9. Scope creep control
10. Lifecycle state persistence and transitions

## Orchestration Loop

Per task loop:

1. `do-task`
2. gate check
3. `audit-task`
4. remediation (if needed)

After all tasks:

1. spec gate (full suite)
2. `run-demo`
3. `audit-spec`

### Runtime Safety Rules

- **Staleness detection**: abort when plan progress does not advance for three
  consecutive cycles.
- **Retry handling**: transient failures back off at 10s/30s/60s; fatal errors
  fail fast.
- **Scope guard**:
  - required implication of spec -> allowed
  - small drive-by fix -> allowed with PR note
  - otherwise -> defer to a new intake issue

## `run-demo` Expectations

`run-demo` verifies behavior, not code style. It should exercise real product
flows (UI/API/data behavior) and report evidence back to the workflow.

## Related Specs

- IPC state machine and transitions: `protocol/PROTOCOL.md`
- Runtime implementation: `../worker-manager-README.md`
