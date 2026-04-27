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

**Why:** Trait-based abstraction enables swapping implementations without changing business logic. Domain crates stay pure and testable without database drivers or HTTP clients in their dependency tree. This is the Rust equivalent of ports-and-adapters architecture.
