# Lane 0.3 — Store Core — Agent Brief

## Task

Implement the event-sourced persistence layer in `crates/tanren-store`. This
crate owns **all SQL, all migrations, and all transactional semantics**. No
other crate in the workspace should contain database queries.

## Full Spec

Read `docs/rewrite/tasks/LANE-0.3-STORE.md` completely before starting. It
contains the complete schema, trait definitions, migration structure, and
test tier requirements. Also read `docs/rewrite/tasks/ADDON-SEAORM.md` for
the rationale behind using SeaORM instead of raw sqlx — the key takeaway is
that SeaORM handles JSON dialect differences (TEXT on SQLite, JSONB on
Postgres) and backend-agnostic migration DDL, but you must still drop to
raw SQL for the atomic dequeue claim path.

## Key Context

- **Clean-room rewrite, not a port.** The Python store in
  `packages/tanren-core/src/tanren_core/store/` exists for conceptual
  reference only. Design for Rust's type system and SeaORM's entity API.
- **Domain types are frozen.** `tanren-domain` is merged and stable. You
  depend on it, never modify it. All entity → domain mapping lives in
  `crates/tanren-store/src/converters.rs`.
- **Both backends must work.** SQLite is the dev/solo target, Postgres is
  the production target. Every migration, every query, every transaction
  must work on both. SeaORM's entity API handles most of this; the
  exception is `JobQueue::dequeue` which has backend-specific paths.
- **Workspace quality rules apply.** Read `CLAUDE.md`. No
  `unwrap`/`todo`/`panic`, no inline `#[allow()]`, max 500 lines per file,
  max 100 lines per function. `thiserror` in the library, never `anyhow`.
  Run `just ci` before considering the work done.
- **Domain compatibility already certified.** `EventEnvelope` round-trips
  through `serde_json::Value` (verified in Lane 0.2 audit). You can
  serialize/deserialize freely.

## Deliverable Modules

| Module | Contents |
|--------|----------|
| `connection.rs` | `connect(url) -> DatabaseConnection` + startup migration runner |
| `migration/` | SeaORM `MigrationTrait` impls for events + dispatch_projection + step_projection |
| `entity/` | One file per table with `DeriveEntityModel` — `events.rs`, `dispatch_projection.rs`, `step_projection.rs`, `mod.rs` |
| `converters.rs` | `From`/`TryFrom` between entity `Model`s and domain types |
| `event_store.rs` | `EventStore` trait + impl using entity API |
| `job_queue.rs` | `JobQueue` trait + impl; raw-SQL dequeue branches on `DbBackend` |
| `state_store.rs` | `StateStore` trait + impl for dispatch/step projection queries |
| `store.rs` | Unified `Store` struct implementing all three traits over one `DatabaseConnection` |
| `errors.rs` | `StoreError` enum (thiserror), with `From<DbErr>` and domain-error mapping |

## Critical Correctness Requirements

1. **Atomic dequeue.** `JobQueue::dequeue` must be race-safe under concurrent
   workers. Postgres path uses `SELECT … FOR UPDATE SKIP LOCKED`, SQLite path
   uses a serializable transaction with `busy_timeout`. Must check
   `count(status='running' AND lane=X) < max_concurrent` and claim the step
   in a **single transaction** to prevent TOCTOU.

2. **Atomic ack_and_enqueue.** Updating the current step's status +
   appending completion events + inserting the next step's row + appending
   `StepEnqueued` must happen in one transaction. Partial success is a bug.

3. **Event append + projection update are co-transactional.** When the
   store emits a `DispatchCreated` event and creates the dispatch
   projection row, both happen in the same transaction or neither does.

4. **Migration idempotency.** `run_migrations` applied twice in a row must
   be a no-op on the second call.

5. **No scan-heavy hot paths.** Every operational query (get_dispatch,
   get_steps_for_dispatch, count_running_steps, query_dispatches with
   filters) must use an index. Check `EXPLAIN` on both backends.

## Testing Strategy

Follow the tier model from the spec:

| Tier | Backend | Framework |
|------|---------|-----------|
| Unit | None | `sea_orm::MockDatabase` for trait logic |
| Integration (dev) | SQLite `:memory:` | `sea_orm::Database::connect("sqlite::memory:")` |
| Integration (prod) | Postgres real | `testcontainers-modules::postgres::Postgres`, gated behind `postgres-integration` feature |
| Concurrency | Postgres real | testcontainers — SQLite cannot exercise `FOR UPDATE SKIP LOCKED` |

Minimum test coverage:
- Every trait method on both backends (SQLite + Postgres integration)
- Round-trip: append events → query events → verify identical
- Race test: spawn N concurrent dequeue tasks on Postgres, assert no double-claim
- Transaction test: `ack_and_enqueue` with a simulated failure mid-op does not leave partial state
- Migration test: apply migrations to fresh DB, verify schema; apply again, verify no-op

## Done When

1. `just ci` passes (full workspace)
2. Integration tests pass on both SQLite and Postgres (via `cargo nextest run -p tanren-store --features postgres-integration` with a running Postgres)
3. All three traits (`EventStore`, `JobQueue`, `StateStore`) are implemented on the unified `Store`
4. Migration framework applies cleanly to a fresh SQLite DB and a fresh Postgres DB
5. Dequeue is race-safe under concurrent access (verify with the concurrency test)
6. `ack_and_enqueue` is fully atomic (verify with the transaction test)
7. `cargo doc -p tanren-store` builds with no warnings
8. No `unwrap()`, `todo!()`, `panic!()` in any code path

## Out of Scope

- User and API key projection tables (Phase 3 — policy lane)
- VM assignment table (Phase 1 — runtime lane)
- Observability instrumentation (Phase 5)
- Read-model optimizations beyond what the base schema requires (Phase 5)
