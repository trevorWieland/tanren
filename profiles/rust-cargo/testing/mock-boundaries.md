# Mock Boundaries

Mock at trait boundaries only. Never mock standard library types or internal implementation details. Use dependency injection.

```rust
// ✓ Good: Define a trait for the external dependency
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}

// In production: inject SystemClock
// In tests: inject a mock
pub struct Scheduler<C: Clock> {
    clock: C,
    // ...
}
```

```rust
// ✓ Good: Mock the trait in tests
struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> { self.0 }
}

#[test]
fn schedules_task_at_next_window() {
    let clock = FixedClock(Utc.with_ymd_and_hms(2025, 1, 15, 9, 0, 0).unwrap());
    let scheduler = Scheduler::new(clock);
    let next = scheduler.next_window();
    assert_eq!(next.hour(), 10);
}
```

```rust
// ✗ Bad: Mocking internals
// Don't mock reqwest::Client directly — use wiremock instead
// Don't mock std::fs functions — use a trait abstraction
// Don't mock private helper functions
```

**Dependency injection patterns:**

```rust
// ✓ Good: Constructor injection with impl Trait
pub fn new_service(
    store: impl EventStore,
    notifier: impl Notifier,
    clock: impl Clock,
) -> Service { /* ... */ }

// ✓ Good: Constructor injection with dyn Trait (when needed for object safety)
pub fn new_service(
    store: Box<dyn EventStore>,
) -> Service { /* ... */ }
```

**Per-tier mock rules:**
- **Unit tests**: mock trait dependencies freely; focus on isolated logic
- **Integration tests**: use real implementations where possible; wiremock for HTTP, in-memory databases
- **Builder pattern for test data**: create test fixture factories, not mock objects

```rust
// ✓ Good: Test data factory
fn test_dispatch() -> Dispatch {
    Dispatch {
        id: DispatchId::new(),
        status: DispatchStatus::Pending,
        created_at: Utc::now(),
        ..Default::default()
    }
}
```

**Why:** Mocking at trait boundaries tests real behavior while isolating external dependencies. Mocking internals creates brittle tests that break on refactoring. Dependency injection keeps production code testable without test-specific conditional logic.
