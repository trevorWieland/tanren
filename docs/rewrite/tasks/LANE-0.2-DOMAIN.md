# Lane 0.2 — Domain Model

## Goal

Implement the canonical domain model in `crates/tanren-domain`. This is the
foundation crate — every other crate depends on it, nothing depends on it.
Must be complete before Lanes 0.3 and 0.4 can start.

## Crate

`crates/tanren-domain/src/lib.rs` and submodules.

## Design Constraints

- **No external runtime or storage concerns** — pure domain logic only
- **No async** — synchronous types and validation only
- **All types `Send + Sync`** — safe for concurrent use
- **`thiserror` for errors** — typed, not stringly-typed
- **`serde` for serialization** — all domain types derive Serialize/Deserialize
- **Newtype IDs** — never use raw `String` or `Uuid` as entity identifiers
- This is a **clean-room redesign**, not a port. Learn from the Python model but
  design for Rust's type system. Use enums with data, builder patterns, and
  compile-time state machine enforcement where it makes sense.

## Deliverables

### 1. ID Newtypes (`ids.rs`)

Strongly-typed identifiers wrapping `uuid::Uuid` (v7 for time-ordered):

```rust
DispatchId, StepId, LeaseId, UserId, TeamId, ApiKeyId, ProjectId, EventId
```

Each should: derive Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize;
implement Display (delegate to inner Uuid); have a `new()` that generates v7.

### 2. Lifecycle Enums (`status.rs`)

**Dispatch lifecycle:**
```
Pending → Running → Completed | Failed | Cancelled
```

**Step lifecycle:**
```
Pending → Running → Completed | Failed | Cancelled
```

**Lease lifecycle** (new in 2.0 — from CONTAINER_SYSTEM.md):
```
Requested → Provisioning → Ready → Running → Idle → Draining → Released
Error path: * → Failed
Cancel path: Running|Ready → Draining → Released
```

Each status enum should have:
- `is_terminal(&self) -> bool`
- `can_transition_to(&self, next: &Self) -> bool` (validates legal transitions)

Also define:
- `DispatchMode`: Auto, Manual
- `StepType`: Provision, Execute, Teardown, DryRun
- `Lane`: Impl, Audit, Gate
- `Phase`: DoTask, AuditTask, RunDemo, AuditSpec, Investigate, Gate, Setup, Cleanup
- `Cli`: Claude, Codex, OpenCode, Bash
- `AuthMode`: ApiKey, OAuth, Subscription
- `Outcome`: Success, Fail, Blocked, Error, Timeout

Add a `cli_to_lane(cli: &Cli) -> Lane` function (Claude/OpenCode→Impl, Codex→Audit, Bash→Gate).

### 3. Commands (`commands.rs`)

Commands are the write-side inputs to the orchestrator. Define as structs:

- `CreateDispatch` — fields: project, phase, cli, auth_mode, branch, spec_folder,
  workflow_id, mode (DispatchMode), timeout, environment_profile, gate_cmd (Option),
  context (Option), model (Option), project_env, required_secrets, preserve_on_failure
- `EnqueueStep` — fields: dispatch_id, step_type, lane (Option), payload (StepPayload enum)
- `CancelDispatch` — fields: dispatch_id, reason (Option)
- `RequestLease` — fields: dispatch_id, step_id, capabilities (LeaseCapabilities), policy_context
- `ReleaseLease` — fields: lease_id, reason (Option)

### 4. Events (`events.rs`)

Events are the canonical history. Define as an enum with data:

```rust
pub enum DomainEvent {
    DispatchCreated { dispatch_id, dispatch, mode, lane, user_id, timestamp },
    DispatchCompleted { dispatch_id, outcome, total_duration_secs, timestamp },
    DispatchFailed { dispatch_id, outcome, failed_step_id, failed_step_type, error, timestamp },
    DispatchCancelled { dispatch_id, reason, timestamp },

    StepEnqueued { dispatch_id, step_id, step_type, step_sequence, lane, timestamp },
    StepDequeued { dispatch_id, step_id, worker_id, timestamp },
    StepStarted { dispatch_id, step_id, worker_id, step_type, timestamp },
    StepCompleted { dispatch_id, step_id, step_type, duration_secs, result_payload, timestamp },
    StepFailed { dispatch_id, step_id, step_type, error, error_class, retry_count, duration_secs, timestamp },

    LeaseRequested { lease_id, dispatch_id, step_id, capabilities, timestamp },
    LeaseProvisioned { lease_id, runtime_type, timestamp },
    LeaseReleased { lease_id, duration_secs, timestamp },
    LeaseFailed { lease_id, error, timestamp },

    PolicyDecision { dispatch_id, decision_type, allowed, reason, timestamp },
}
```

Each variant should have a `dispatch_id()` accessor method for filtering.
Consider a shared `EventEnvelope` wrapper: `{ event_id: EventId, timestamp: DateTime<Utc>, entity_id: String, entity_type: EntityType, payload: DomainEvent }`.

### 5. Step Payloads (`payloads.rs`)

Input payloads (what the worker receives):
- `ProvisionPayload` — embedded dispatch snapshot (transition-compatible form)
- `ProvisionRefPayload` — typed dispatch snapshot reference (`dispatch_id`)
- `ExecutePayload` — dispatch snapshot + environment handle
- `TeardownPayload` — dispatch snapshot + environment handle + preserve flag
- `DryRunPayload` — dispatch snapshot

Result payloads (what the worker produces):
- `ProvisionResult` — environment handle
- `ExecuteResult` — outcome, signal, exit_code, duration_secs, gate_output, tail_output,
  stderr_tail, pushed, plan_hash, unchecked_tasks, spec_modified, findings, token_usage
- `TeardownResult` — vm_released, duration_secs, estimated_cost
- `DryRunResult` — provider, server_type, estimated_cost_hourly, would_provision

Use `StepPayload` and `StepResult` enums to wrap these.

### 6. Error Taxonomy (`errors.rs`)

**Domain errors** (what the orchestrator returns):
```rust
pub enum DomainError {
    // Guard violations
    ConcurrentExecute { dispatch_id },
    PostTeardownExecute { dispatch_id },
    ActiveExecuteTeardown { dispatch_id },
    DuplicateTeardown { dispatch_id },

    // Policy
    PolicyDenied { reason, decision },
    BudgetExceeded { limit, current },
    QuotaExhausted { resource, limit },

    // Preconditions
    NotFound { entity_type, id },
    InvalidTransition { from, to, entity_type, id },
    PreconditionFailed { reason },

    // Conflict
    Conflict { reason },
}
```

**Error classification** (for retry decisions):
```rust
pub enum ErrorClass { Transient, Fatal, Ambiguous }
```

With `classify_error(exit_code, stdout, stderr, signal) -> ErrorClass` function
and `TRANSIENT_BACKOFF: [u64; 3] = [10, 30, 60]`.

### 7. View Types (`views.rs`)

Read-side projection types (what queries return):
- `DispatchView` — dispatch_id, mode, status, outcome, lane, dispatch snapshot,
  user_id, created_at, updated_at
- `StepView` — step_id, dispatch_id, step_type, step_sequence, lane, status,
  worker_id, payload, result, error, retry_count, created_at, updated_at
- `EventQueryResult` — events (Vec), total_count, has_more

### 8. Guard Logic (`guards.rs`)

Pure functions (no DB) that validate state transitions from a list of steps:

- `check_execute_guards(steps: &[StepView]) -> Result<(), DomainError>`
- `check_teardown_guards(steps: &[StepView], allow_retry_after_failure: bool) -> Result<(), DomainError>`

This keeps the guard logic in domain where it belongs, testable without a store.

## Testing

- **Unit tests** for every state machine transition (valid and invalid)
- **Property tests** (`proptest`) for state machine invariants: generate random
  sequences of transitions and verify `can_transition_to` is consistent
- **Snapshot tests** (`insta`) for serde round-trips of all event variants
- Aim for high coverage — this crate is the contract everything else depends on

## Exit Criteria

- `cargo test -p tanren-domain` passes with comprehensive coverage
- `cargo clippy -p tanren-domain` clean with workspace lints
- `cargo doc -p tanren-domain` builds with no warnings
- All domain types are `Send + Sync + Clone + Debug + Serialize + Deserialize`
- Guard logic has 100% branch coverage on valid/invalid transitions
- No `unwrap()`, `todo!()`, `panic!()` in any code path

## Reference (Do NOT Port)

The Python domain model lives in these files for conceptual reference only:
- `packages/tanren-core/src/tanren_core/schemas.py` — Dispatch, Phase, Cli, Outcome, etc.
- `packages/tanren-core/src/tanren_core/store/enums.py` — DispatchStatus, StepStatus, Lane, etc.
- `packages/tanren-core/src/tanren_core/store/events.py` — event types
- `packages/tanren-core/src/tanren_core/store/payloads.py` — step payloads
- `packages/tanren-core/src/tanren_core/errors.py` — error classification
- `packages/tanren-core/src/tanren_core/dispatch_orchestrator.py` — guard rules

Key differences from Python to design for:
- Use Rust enums with data instead of class hierarchies
- Use newtype IDs instead of raw strings
- Enforce state transitions at the type level where feasible
- Lease lifecycle is new (Python only has provision/teardown, no formal lease model)
- Policy decisions are new first-class events (Python has scattered conditionals)
