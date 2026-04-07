# Lane 0.3 — Store Core

## Goal

Implement the event-sourced persistence layer in `crates/tanren-store`. This
crate owns all SQL, all migrations, and all transactional semantics. No other
crate should contain database queries.

**Depends on:** Lane 0.2 (tanren-domain) — needs ID types, events, status enums,
view types, and payloads.

**Can run in parallel with:** Lane 0.4 (contract + CLI wiring).

## Crate

`crates/tanren-store/src/lib.rs` and submodules.

## Design Constraints

- **`sqlx` with compile-time checked queries** where feasible
- **Support both `SQLite` (local/dev) and Postgres (team/enterprise)**
- **Async throughout** — all store operations are async (tokio runtime)
- **Transactional guarantees** — event append + projection update must be atomic
- **No domain logic** — the store persists and queries, it doesn't decide
- **Migration framework** integrated into startup and CI

## Deliverables

### 1. Database Engine (`engine.rs`)

- `create_pool(url: &str) -> Result<sqlx::AnyPool>` — connect to SQLite or Postgres
  based on URL scheme
- Connection pool configuration (max connections, idle timeout, etc.)
- Startup migration runner (apply pending migrations before accepting queries)

### 2. Migrations (`migrations/`)

Use sqlx's built-in migration system. Initial migration creates:

**`events` table** (append-only log):
```sql
CREATE TABLE events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,  -- BIGSERIAL for Postgres
    event_id    TEXT NOT NULL UNIQUE,
    timestamp   TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    entity_type TEXT NOT NULL DEFAULT 'dispatch',
    event_type  TEXT NOT NULL,
    payload     TEXT NOT NULL,  -- JSONB for Postgres
    -- Indexes
);
CREATE INDEX idx_events_entity_id ON events(entity_id);
CREATE INDEX idx_events_entity_type ON events(entity_type);
CREATE INDEX idx_events_event_type ON events(event_type);
CREATE INDEX idx_events_timestamp ON events(timestamp);
```

**`dispatch_projection` table**:
```sql
CREATE TABLE dispatch_projection (
    dispatch_id         TEXT PRIMARY KEY,
    mode                TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending',
    outcome             TEXT,
    lane                TEXT NOT NULL,
    preserve_on_failure INTEGER NOT NULL DEFAULT 0,  -- BOOLEAN for Postgres
    dispatch_json       TEXT NOT NULL,                -- JSONB for Postgres
    user_id             TEXT NOT NULL DEFAULT '',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE INDEX idx_dispatch_status ON dispatch_projection(status);
CREATE INDEX idx_dispatch_lane ON dispatch_projection(lane);
CREATE INDEX idx_dispatch_created ON dispatch_projection(created_at);
CREATE INDEX idx_dispatch_user ON dispatch_projection(user_id);
```

**`step_projection` table**:
```sql
CREATE TABLE step_projection (
    step_id         TEXT PRIMARY KEY,
    dispatch_id     TEXT NOT NULL REFERENCES dispatch_projection(dispatch_id),
    step_type       TEXT NOT NULL,
    step_sequence   INTEGER NOT NULL,
    lane            TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    worker_id       TEXT,
    payload_json    TEXT NOT NULL,   -- JSONB for Postgres
    result_json     TEXT,            -- JSONB for Postgres
    error           TEXT,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX idx_step_dispatch ON step_projection(dispatch_id);
CREATE INDEX idx_step_status ON step_projection(status);
CREATE INDEX idx_step_lane_status ON step_projection(lane, status);
```

**Note:** User/ApiKey projection tables will be added in Phase 3 (policy lane).
For now, include them as empty stubs in migration comments if helpful, but don't
implement the read/write paths yet.

### 3. Event Store (`event_store.rs`)

Trait + implementation:

```rust
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append an event to the log. Must be transactional with any
    /// projection updates that happen in the same logical operation.
    async fn append(&self, event: &EventEnvelope) -> Result<()>;

    /// Append multiple events atomically.
    async fn append_batch(&self, events: &[EventEnvelope]) -> Result<()>;

    /// Query events with filters and pagination.
    async fn query_events(&self, filter: &EventFilter) -> Result<EventQueryResult>;
}
```

`EventFilter` fields: entity_id, entity_ids (Vec), entity_type, event_type,
since (DateTime), until (DateTime), limit, offset.

Implementation: `SqlEventStore` backed by `sqlx::AnyPool`.

### 4. Job Queue (`job_queue.rs`)

Trait + implementation:

```rust
#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Enqueue a step. Appends StepEnqueued event atomically.
    async fn enqueue_step(&self, params: EnqueueStepParams) -> Result<()>;

    /// Atomically claim a pending step for a worker.
    /// Returns None if no work available or lane at capacity.
    async fn dequeue(&self, params: DequeueParams) -> Result<Option<QueuedStep>>;

    /// Mark step completed and store result.
    async fn ack(&self, step_id: &StepId, result_json: &str) -> Result<()>;

    /// Atomically ack current step and enqueue next step.
    /// Used for auto-chaining (provision → execute → teardown).
    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> Result<()>;

    /// Cancel all pending (non-teardown) steps for a dispatch.
    async fn cancel_pending_steps(&self, dispatch_id: &DispatchId) -> Result<u64>;

    /// Mark step failed. If retry=true, increment retry_count and reset to pending.
    async fn nack(&self, step_id: &StepId, params: NackParams) -> Result<()>;

    /// Reset stale running steps back to pending (crash recovery).
    async fn recover_stale_steps(&self, timeout_secs: u64) -> Result<u64>;
}
```

Critical: `dequeue` must be atomic — check `count(status='running' AND lane=X) < max_concurrent`
and claim in a single transaction to prevent TOCTOU races.

Critical: `ack_and_enqueue` must be a single transaction — ack the current step,
enqueue the next, and optionally append completion events, all atomically.

### 5. State Store (`state_store.rs`)

Trait + implementation:

```rust
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get_dispatch(&self, id: &DispatchId) -> Result<Option<DispatchView>>;
    async fn query_dispatches(&self, filter: &DispatchFilter) -> Result<Vec<DispatchView>>;
    async fn get_step(&self, id: &StepId) -> Result<Option<StepView>>;
    async fn get_steps_for_dispatch(&self, dispatch_id: &DispatchId) -> Result<Vec<StepView>>;
    async fn count_running_steps(&self, lane: Option<&Lane>) -> Result<u64>;
    async fn create_dispatch_projection(&self, params: CreateDispatchParams) -> Result<()>;
    async fn update_dispatch_status(&self, id: &DispatchId, status: &DispatchStatus, outcome: Option<&Outcome>) -> Result<()>;
}
```

### 6. Unified Store (`store.rs`)

A single `Store` struct that implements all three traits, backed by one connection pool:

```rust
pub struct Store {
    pool: sqlx::AnyPool,
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self>;
    pub async fn run_migrations(&self) -> Result<()>;
    pub async fn close(&self) -> Result<()>;
}

impl EventStore for Store { ... }
impl JobQueue for Store { ... }
impl StateStore for Store { ... }
```

### 7. Converters (`converters.rs`)

Map between database rows and domain view types:
- `row_to_dispatch_view(row) -> DispatchView`
- `row_to_step_view(row) -> StepView`
- `row_to_event_envelope(row) -> EventEnvelope`

## Testing

- **Unit tests** with in-memory SQLite for all trait methods
- **Integration tests** with Postgres (gated behind `postgres` feature/marker)
- **Concurrency tests**: spawn multiple dequeue tasks, verify no double-claims
- **Transaction tests**: verify ack_and_enqueue is atomic (simulate crash mid-op)
- **Migration tests**: apply migrations to fresh DB, verify schema matches expectations
- **Round-trip tests**: append events → query events → verify identical

## Exit Criteria

- `cargo test -p tanren-store` passes with SQLite backend
- All three trait implementations (EventStore, JobQueue, StateStore) are complete
- Migrations apply cleanly to fresh SQLite database
- Dequeue is race-safe under concurrent access
- ack_and_enqueue is fully atomic
- `cargo clippy -p tanren-store` clean with workspace lints
- No `unwrap()`, `todo!()`, `panic!()` in any code path

## Reference (Do NOT Port)

Python store implementation for conceptual reference:
- `packages/tanren-core/src/tanren_core/store/repository.py` — unified Store class
- `packages/tanren-core/src/tanren_core/store/models.py` — SQLAlchemy ORM models
- `packages/tanren-core/src/tanren_core/store/protocols.py` — trait equivalents
- `packages/tanren-core/src/tanren_core/store/engine.py` — async engine creation
- `packages/tanren-core/src/tanren_core/store/converters.py` — row → view mappers

Key differences from Python:
- Use sqlx compile-time checked queries, not SQLAlchemy ORM
- Use `sqlx::AnyPool` for SQLite/Postgres polymorphism
- Dequeue must use proper transaction isolation (Python had TOCTOU bugs that were fixed)
- Store migrations via sqlx's built-in system, not Alembic
