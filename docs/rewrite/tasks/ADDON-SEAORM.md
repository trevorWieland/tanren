# Add-on Brief — Switch Store Layer from sqlx to SeaORM

## Scope

This is a delta, not a rewrite. It supersedes the sqlx-specific guidance in
`LANE-0.3-STORE.md`. Everything else in that brief (trait shape, guard
semantics, transactional contracts, test coverage requirements) stands.

## Rationale

- **JSON dialect split:** SQLite stores JSON as `TEXT`, Postgres as `JSONB`.
  SeaORM handles this transparently via `serde_json::Value` columns.
- **Migration dialect split:** SeaORM Migration emits correct DDL per backend
  from one schema definition; raw sqlx forces hand-maintained dialects.
- **Compile-time checking actually works:** `sqlx::query!` macros require a
  live DB at build time and don't function through `AnyPool`. SeaORM's
  type-level checking works regardless of runtime backend.
- **Single source of truth:** Entity definitions double as schema docs.

## Workspace — `Cargo.toml`

Remove `sqlx` from `[workspace.dependencies]`. Add:

```toml
sea-orm = { version = "1.1", features = [
  "runtime-tokio-rustls",
  "sqlx-sqlite",
  "sqlx-postgres",
  "with-chrono",
  "with-uuid",
  "with-json",
  "macros",
] }
sea-orm-migration = { version = "1.1", features = [
  "runtime-tokio-rustls",
  "sqlx-sqlite",
  "sqlx-postgres",
] }
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres"] }
```

SeaORM pulls sqlx in as a transitive dep through its feature flags — no
duplicate dependency.

## `LANE-0.3-STORE.md` — sections to replace

### `engine.rs` → `connection.rs`

```rust
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr>;
```

`DatabaseConnection` replaces `sqlx::AnyPool` as the single handle threaded
through `EventStore`, `JobQueue`, and `StateStore`. Accepts the same
`sqlite://…` and `postgres://…` URL schemes.

### `migrations/` → `migration/` module

Each migration is a `MigrationTrait` impl. Use `SchemaManager::create_table`,
`create_index`, etc. — backend-agnostic, emits correct DDL per backend.
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
            .col(ColumnDef::new(Events::Payload).json_binary().not_null())  // TEXT on SQLite, JSONB on Postgres
            .to_owned(),
    )
    .await?;
```

SeaORM's `json_binary()` column type is the magic piece: emits `TEXT` on
SQLite and `JSONB` on Postgres without caller intervention.

### `models.rs` → `entity/` module

One file per table:

```
entity/
  events.rs
  dispatch_projection.rs
  step_projection.rs
  mod.rs
```

Each file defines:

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
`tanren-domain`. The store maps between entity `Model` and domain types via
`From`/`TryFrom` impls in a `converters.rs` module — same boundary the
original brief called for.

### `event_store.rs`, `state_store.rs` — use entity API

Replace hand-rolled SQL with SeaORM query builder:

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
    // map to domain types
}
```

### `job_queue.rs` — keep raw SQL escape hatch for dequeue

**This is the one place where backend-specific raw SQL remains.** SeaORM does
not abstract `SELECT ... FOR UPDATE SKIP LOCKED` vs SQLite's locking model.

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

Every other `JobQueue` method (`enqueue_step`, `ack`, `nack`,
`cancel_pending_steps`, `recover_stale_steps`) uses the entity API normally.
Only the dequeue claim has two paths.

`ack_and_enqueue` becomes:

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

### Testing strategy

Replaces the "in-memory SQLite + marked Postgres" pattern:

| Layer | Backend | Tool |
|-------|---------|------|
| Unit (converters, entity mapping) | none | `sea_orm::MockDatabase` |
| Integration — SQLite | in-memory | `sea_orm::Database::connect("sqlite::memory:")` |
| Integration — Postgres | real | `testcontainers-modules::postgres::Postgres` |
| Concurrency (dequeue race) | Postgres | testcontainers — SQLite can't exercise `SKIP LOCKED` |

Gate the Postgres tests behind a cargo feature (`postgres-integration`) or
a nextest filter so they only run in CI / on demand, not on every local
`just test`.

## `LANE-0.4-CLI-WIRING.md` — no spec change

The CLI wiring brief depends on store **traits**, not implementation. The
only user-visible change is the type name of the handle that the CLI
constructs:

```rust
// Before (implicit in the brief)
let store = Store::new(&args.database_url).await?;  // wraps sqlx::AnyPool

// After
let store = Store::new(&args.database_url).await?;  // wraps sea_orm::DatabaseConnection
```

Same constructor signature, same trait impls, same tests. Zero brief changes
needed.

## `LANE-0.2-DOMAIN.md` — no change

Domain crate has no database dependency. SeaORM lives entirely in
`tanren-store`.

## `rust-ci.yml` — add Postgres integration job

New job alongside the existing `test` job:

```yaml
integration-postgres:
  name: Integration (Postgres)
  runs-on: ubuntu-latest
  services:
    postgres:
      image: postgres:16-alpine
      env:
        POSTGRES_PASSWORD: tanren
      options: >-
        --health-cmd pg_isready
        --health-interval 10s
        --health-timeout 5s
        --health-retries 5
      ports:
        - 5432:5432
  steps:
    - uses: actions/checkout@v6
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
    - uses: taiki-e/install-action@v2
      with:
        tool: cargo-nextest
    - name: Run Postgres integration tests
      env:
        TANREN_TEST_POSTGRES_URL: postgres://postgres:tanren@localhost:5432/postgres
      run: cargo nextest run -p tanren-store --features postgres-integration
```

Use a GitHub Actions `services:` container in CI (faster than
testcontainers for CI) and use `testcontainers` for local `just` runs.
Both point at the same test suite via `TANREN_TEST_POSTGRES_URL`.

## `deny.toml` — no change

SeaORM and testcontainers are permissive-licensed (MIT/Apache-2.0).
Verified against the existing allowlist.

## Checklist for the operator making this change

- [ ] Swap `Cargo.toml` workspace dependencies
- [ ] Apply the `LANE-0.3-STORE.md` sectional edits listed above
- [ ] Add the integration-postgres CI job to `rust-ci.yml`
- [ ] Run `just ci` to verify the workspace still builds with the new deps
      (crates other than `tanren-store` should be unaffected)
- [ ] Commit as a single prep change before the Lane 0.3 agent picks up the work
