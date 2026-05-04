---
kind: standard
name: trait-based-abstraction
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# Trait-Based Abstraction

Access infrastructure through traits defined in core crates. Never import concrete implementations in domain logic. Wire implementations via constructor injection.

```rust
// ✓ Good: Port trait in domain crate
// crates/myapp-domain/src/ports.rs
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, events: &[Event]) -> Result<(), StoreError>;
    async fn replay(&self, stream_id: StreamId) -> Result<Vec<Event>, StoreError>;
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, notification: Notification) -> Result<(), NotifyError>;
}
```

```rust
// ✓ Good: Adapter implementation in infrastructure crate
// crates/myapp-store/src/sqlite.rs
use myapp_domain::ports::EventStore;

pub struct SqliteEventStore { pool: SqlitePool }

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn append(&self, events: &[Event]) -> Result<(), StoreError> {
        // SQLite-specific implementation
    }
    // ...
}
```

```rust
// ✓ Good: Constructor injection in binary crate
// bin/myapp-api/src/main.rs
let store = SqliteEventStore::new(&config.database_url).await?;
let notifier = WebhookNotifier::new(&config.webhook_url);
let service = OrchestratorService::new(store, notifier);
```

```rust
// ✗ Bad: Direct infrastructure import in domain
// crates/myapp-domain/src/orchestrator.rs
use sqlx::SqlitePool;  // Domain crate importing database driver!

pub struct Orchestrator {
    pool: SqlitePool,  // Concrete dependency in domain
}
```

**Rules:**
- Core/domain crate: defines port traits, domain types, business logic. Zero I/O dependencies.
- Infrastructure crates: implement port traits with concrete backends (SQLite, Postgres, HTTP, etc.)
- Binary crates: compose the dependency graph, inject implementations
- Use `async_trait` for async trait methods (until native async traits stabilize fully)
- Prefer `impl Trait` for constructor parameters; use `Box<dyn Trait>` when object safety requires it

**Swapping implementations:**
- Same trait, different backend: `SqliteEventStore` vs `PostgresEventStore`
- Testing: inject mock implementations in tests, real implementations in production
- No conditional compilation needed — the type system handles it

## Worked example: `AccountStore` (R-0001)

The store crate `tanren-store` defines a single port trait covering every
account-flow read and write — account, membership, session, event, and
invitation surfaces — so app-services never reaches into a concrete backend.

```rust
// ✓ Good: port trait in tanren-store
// crates/tanren-store/src/lib.rs
pub trait AccountStore: Send + Sync + std::fmt::Debug {
    // accounts
    async fn create_account(&self, ...) -> Result<Account, StoreError>;
    async fn find_account_by_email(&self, ...) -> Result<Option<Account>, StoreError>;

    // memberships
    async fn upsert_membership(&self, ...) -> Result<Membership, StoreError>;

    // sessions
    async fn create_session(&self, ...) -> Result<Session, StoreError>;
    async fn revoke_session(&self, ...) -> Result<(), StoreError>;

    // account events
    async fn append_event(&self, ...) -> Result<(), StoreError>;

    // invitations
    async fn create_invitation(&self, ...) -> Result<Invitation, StoreError>;
    async fn redeem_invitation(&self, ...) -> Result<Invitation, StoreError>;
}
```

```rust
// ✓ Good: SeaORM is the production adapter
// crates/tanren-store/src/sea_orm.rs
pub struct SeaOrmStore { db: DatabaseConnection }

impl AccountStore for SeaOrmStore {
    // SeaORM-specific implementation
}
```

```rust
// ✓ Good: app-services depends only on the trait
// crates/tanren-app-services/src/sign_up.rs
pub async fn sign_up(
    store: &dyn AccountStore,
    request: SignUpRequest,
) -> Result<Account, SignUpError> {
    // no SeaORM types reachable from here
}
```

```rust
// ✗ Bad: concrete struct leaking into app-services
use tanren_store::SeaOrmStore;          // disallowed_types — clippy rejects
pub fn make_service(s: SeaOrmStore) {}
```

The concrete struct is forbidden in app-services and BDD via per-crate
`clippy.toml`:

```toml
# crates/tanren-app-services/clippy.toml
disallowed_types = ["tanren_store::SeaOrmStore"]

# crates/tanren-bdd/clippy.toml
disallowed_types = ["tanren_store::SeaOrmStore"]
```

Only the binary library crates (`tanren-{api,cli,mcp,tui}-app`) are allowed
to name `SeaOrmStore` directly — that is where wiring happens.

**Why:** Trait-based abstraction enables swapping implementations without changing business logic. Domain crates stay pure and testable without database drivers or HTTP clients in their dependency tree. This is the Rust equivalent of ports-and-adapters architecture.
