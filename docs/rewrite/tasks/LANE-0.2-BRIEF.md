# Lane 0.2 — Domain Model — Agent Brief

## Task

Implement the canonical domain model in `crates/tanren-domain`. This is the
leaf crate of the workspace — no internal dependencies, everything else depends
on it.

## Full Spec

Read `docs/rewrite/tasks/LANE-0.2-DOMAIN.md` completely before starting. It
contains the complete list of types, enums, state machines, events, errors, and
design constraints.

## Key Context

- **This is a clean-room rewrite, not a port.** The Python codebase
  (`packages/tanren-core/src/tanren_core/`) exists for conceptual reference only.
  Design for Rust's type system — enums with data, newtypes, compile-time
  guarantees where feasible.
- **Forgeclaw is the first consumer** of tanren. The domain types will be used
  across process boundaries, so serde contracts matter.
- **Workspace quality rules apply.** Read `CLAUDE.md` for conventions: no
  `unwrap`/`todo`/`panic`, no inline `#[allow()]`, max 500 lines per file, max
  100 lines per function. Run `just ci` before considering the work done.

## Deliverable Modules

| Module | Contents |
|--------|----------|
| `ids.rs` | Newtype ID wrappers around `uuid::Uuid` v7 |
| `status.rs` | Lifecycle enums with `is_terminal()` and `can_transition_to()` + all value enums (Phase, Cli, Lane, etc.) |
| `commands.rs` | Write-side command structs (CreateDispatch, EnqueueStep, CancelDispatch, RequestLease, ReleaseLease) |
| `events.rs` | `DomainEvent` enum + `EventEnvelope` wrapper |
| `payloads.rs` | Step input payloads and result payloads |
| `errors.rs` | `DomainError` enum + `ErrorClass` + `classify_error()` |
| `views.rs` | Read-side projection types (DispatchView, StepView, EventQueryResult) |
| `guards.rs` | Pure guard functions operating on `&[StepView]` |

## Done When

1. `just ci` passes (full workspace, not just this crate)
2. All types are `Send + Sync + Clone + Debug + Serialize + Deserialize`
3. State machine transitions have property tests (`proptest`)
4. All event variants have serde round-trip snapshot tests (`insta`)
5. Guard logic has full branch coverage
6. `cargo doc -p tanren-domain` builds cleanly
