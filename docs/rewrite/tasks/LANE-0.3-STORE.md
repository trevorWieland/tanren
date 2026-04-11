# Lane 0.3 — Store Core

## Goal

Implement the event-sourced persistence layer in `crates/tanren-store`. This
crate owns all SQL, all migrations, and all transactional semantics. No other
crate should contain database queries.

**Depends on:** Lane 0.2 (tanren-domain) — needs ID types, events, status enums,
view types, and payloads.

**Can run in parallel with:** Lane 0.4 (contract + CLI wiring).

> **Add-on applied:** this brief has been updated to use **SeaORM** in place
> of raw `sqlx`. See `ADDON-SEAORM.md` for rationale. The trait shape, guard
> semantics, transactional contracts, and test coverage requirements are
> unchanged — only the persistence mechanism differs.

## Crate

`crates/tanren-store/src/lib.rs` and submodules.

## Design Constraints

- **SeaORM** as the persistence layer (pulls `sqlx` in transitively)
- **Support both `SQLite` (local/dev) and Postgres (team/enterprise)** via one
  `DatabaseConnection` handle
- **Async throughout** — all store operations are async (tokio runtime)
- **Transactional guarantees** — event append + projection update must be atomic
- **No domain logic** — the store persists and queries, it doesn't decide
- **Migration framework** integrated into startup and CI, backend-agnostic

## Deliverables

### 1. Connection (`connection.rs`)

```rust
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr>;
```

`DatabaseConnection` is the single handle threaded through `EventStore`,
`JobQueue`, and `StateStore`. Accepts `sqlite://…` and `postgres://…` URL
schemes. Connection pool configuration (max connections, idle timeout,
etc.) is expressed through SeaORM's `ConnectOptions`. Startup migration
runner applies pending migrations before accepting queries.

### 2. Migrations (`migration/` module)

Each migration is a `MigrationTrait` impl using `SchemaManager`. The
`json_binary()` column type is the key piece: emits `TEXT` on SQLite and
`JSONB` on Postgres from a single definition.

Example for the `events` table:

```rust
manager
    .create_table(
        Table::create()
            .table(Events::Table)
            .col(ColumnDef::new(Events::Id).big_integer().not_null().auto_increment().primary_key())
            .col(ColumnDef::new(Events::EventId).uuid().not_null().unique_key())
            .col(ColumnDef::new(Events::Timestamp).timestamp_with_time_zone().not_null())
            .col(ColumnDef::new(Events::EntityKind).string().not_null())
            .col(ColumnDef::new(Events::EntityId).string().not_null())
            .col(ColumnDef::new(Events::EventType).string().not_null())
            .col(ColumnDef::new(Events::SchemaVersion).integer().not_null())
            .col(ColumnDef::new(Events::Payload).json_binary().not_null())
            .to_owned(),
    )
    .await?;
```

Repeat the pattern for `dispatch_projection` and `step_projection`.
Required indexes:

- `events`: entity_id, entity_kind, event_type, timestamp
- `dispatch_projection`: status, lane, created_at, user_id
- `step_projection`: dispatch_id, status, (lane, status) composite

**Note:** User/ApiKey projection tables will be added in Phase 3 (policy
lane) — leave as stubs for now, don't implement the read/write paths yet.

### 3. Entity models (`entity/` module)

One file per table — `events.rs`, `dispatch_projection.rs`,
`step_projection.rs`, `mod.rs`. Each file defines a SeaORM `DeriveEntityModel`:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,
    #[sea_orm(unique)]
    pub event_id: Uuid,
    pub timestamp: DateTimeUtc,
    pub entity_kind: String,
    pub entity_id: String,
    pub event_type: String,
    pub schema_version: i32,
    #[sea_orm(column_type = "JsonBinary")]
    pub payload: serde_json::Value,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Domain types (`EventEnvelope`, `DispatchView`, `StepView`) stay in
`tanren-domain`. The store maps between entity `Model`s and domain types
via `From`/`TryFrom` impls in `converters.rs`.

### 4. Event Store (`event_store.rs`)

```rust
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &EventEnvelope) -> Result<()>;
    async fn append_batch(&self, events: &[EventEnvelope]) -> Result<()>;
    async fn query_events(&self, filter: &EventFilter) -> Result<EventQueryResult>;
}
```

`EventFilter` fields: entity_id, entity_ids (Vec), entity_kind, event_type,
since (DateTime), until (DateTime), limit, offset.

Implementation uses the SeaORM entity API:

```rust
async fn append(&self, envelope: &EventEnvelope) -> Result<(), StoreError> {
    let model: events::ActiveModel = envelope.try_into()?;
    model.insert(&self.conn).await?;
    Ok(())
}

async fn query_events(&self, filter: &EventFilter) -> Result<EventQueryResult, StoreError> {
    let mut q = events::Entity::find();
    if let Some(ref entity_id) = filter.entity_id {
        q = q.filter(events::Column::EntityId.eq(entity_id.as_str()));
    }
    // ...
    let rows = q.limit(filter.limit).offset(filter.offset).all(&self.conn).await?;
    // map Model → EventEnvelope via converters.rs
}
```

### 5. Job Queue (`job_queue.rs`)

Trait unchanged:

```rust
#[async_trait]
pub trait JobQueue: Send + Sync {
    async fn enqueue_step(&self, params: EnqueueStepParams) -> Result<()>;
    async fn dequeue(&self, params: DequeueParams) -> Result<Option<QueuedStep>>;
    async fn ack(&self, step_id: &StepId, result_json: &str) -> Result<()>;
    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> Result<()>;
    async fn cancel_pending_steps(&self, dispatch_id: &DispatchId) -> Result<u64>;
    async fn nack(&self, step_id: &StepId, params: NackParams) -> Result<()>;
    async fn recover_stale_steps(&self, timeout_secs: u64) -> Result<u64>;
}
```

Every method uses the entity API **except `dequeue`** — SeaORM does not
abstract `SELECT … FOR UPDATE SKIP LOCKED` vs SQLite's single-writer
locking model, so the claim path branches on the backend:

```rust
async fn dequeue(&self, params: DequeueParams) -> Result<Option<QueuedStep>, StoreError> {
    match self.conn.get_database_backend() {
        DbBackend::Postgres => {
            let stmt = Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE step_projection
                SET status = 'running', worker_id = $1, updated_at = NOW()
                WHERE step_id = (
                    SELECT step_id FROM step_projection
                    WHERE status = 'pending' AND ($2::text IS NULL OR lane = $2)
                    ORDER BY created_at
                    FOR UPDATE SKIP LOCKED
                    LIMIT 1
                )
                RETURNING ...
                "#,
                vec![params.worker_id.into(), params.lane.map(|l| l.to_string()).into()],
            );
            // execute + map
        }
        DbBackend::Sqlite => {
            // Serializable transaction + busy_timeout. Document the
            // concurrency trade-off (single-writer on SQLite is fine).
        }
        DbBackend::MySql => unreachable!("MySQL is not a supported backend"),
    }
}
```

Every other method uses the entity API normally. `dequeue` is the only
place where backend-specific raw SQL remains.

Critical: `dequeue` must be atomic — check `count(status='running' AND lane=X) < max_concurrent`
and claim in a single transaction to prevent TOCTOU races.

Critical: `ack_and_enqueue` must be a single transaction — ack the
current step, enqueue the next, and optionally append completion events,
all atomically. Uses `conn.transaction`:

```rust
self.conn
    .transaction::<_, (), StoreError>(|txn| Box::pin(async move {
        // ack current step (update step_projection row)
        // append completion events (insert into events)
        // insert next step_projection row
        // append StepEnqueued event
        Ok(())
    }))
    .await?;
```

Single transaction, backend-agnostic.

### 6. State Store (`state_store.rs`)

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

### 7. Unified Store (`store.rs`)

```rust
pub struct Store {
    conn: DatabaseConnection,
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self>;
    pub async fn run_migrations(&self) -> Result<()>;
    pub async fn close(self) -> Result<()>;
}

impl EventStore for Store { ... }
impl JobQueue for Store { ... }
impl StateStore for Store { ... }
```

### 8. Converters (`converters.rs`)

Map between SeaORM entity `Model`s and domain view types via `From` /
`TryFrom` impls:
- `events::Model` ↔ `EventEnvelope`
- `dispatch_projection::Model` → `DispatchView`
- `step_projection::Model` → `StepView`

Fallible direction uses `TryFrom` so malformed JSON payloads surface as
`StoreError::ConversionError` rather than panicking.

## Testing

| Layer | Backend | Tool |
|-------|---------|------|
| Unit (converters, entity mapping) | none | `sea_orm::MockDatabase` |
| Integration — SQLite | in-memory | `sea_orm::Database::connect("sqlite::memory:")` |
| Integration — Postgres | real | `testcontainers-modules::postgres::Postgres` |
| Concurrency (dequeue race) | Postgres | testcontainers — SQLite can't exercise `SKIP LOCKED` |

Gate the Postgres integration tests behind a cargo feature
(`postgres-integration`) or a nextest filter so they only run in CI / on
demand, not on every local `just test`.

Additional test coverage:

- **Concurrency tests**: spawn multiple dequeue tasks against Postgres,
  verify no double-claims
- **Transaction tests**: verify `ack_and_enqueue` is atomic (simulate
  crash mid-op)
- **Migration tests**: apply migrations to fresh SQLite and fresh
  Postgres, verify schema matches expectations on both
- **Round-trip tests**: append events → query events → verify identical

## Exit Criteria

- `cargo test -p tanren-store` passes against SQLite backend
- `cargo nextest run -p tanren-store --features postgres-integration`
  passes against a testcontainers-managed Postgres
- All three trait implementations (EventStore, JobQueue, StateStore) are
  complete
- Migrations apply cleanly to fresh SQLite **and** fresh Postgres
- Dequeue is race-safe under concurrent access on Postgres; documented
  as single-writer on SQLite
- `ack_and_enqueue` is fully atomic on both backends
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
- Use SeaORM entity models, not SQLAlchemy ORM
- Use `DatabaseConnection` for SQLite/Postgres polymorphism (sqlx is
  pulled in transitively through SeaORM)
- Dequeue must use proper transaction isolation (Python had TOCTOU bugs
  that were fixed) — Postgres path uses `FOR UPDATE SKIP LOCKED`
- Migrations via SeaORM's `MigrationTrait`, not Alembic
