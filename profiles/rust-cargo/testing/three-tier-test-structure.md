# Three-Tier Test Structure

Tests organized into unit, integration, and doc tests following Rust's native test layout. Each tier has clear boundaries and timing limits.

```
crates/myapp-core/
├── src/
│   ├── dispatch.rs           # Source code
│   └── dispatch.rs           # Unit tests at bottom: #[cfg(test)] mod tests
├── tests/
│   ├── dispatch_lifecycle.rs  # Integration tests: full public API
│   └── common/
│       └── mod.rs            # Shared test utilities
```

**Tier definitions:**

**Unit tests** (`#[cfg(test)] mod tests` in source files):
- Fast: <250ms per test
- Mocks allowed and encouraged for external dependencies
- No I/O, no network, no database
- Test isolated logic, algorithms, and state transitions
- Colocated with the code they test

```rust
// ✓ Good: Unit test at bottom of source file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dispatch_command() {
        let cmd = DispatchCommand::builder()
            .agent_id(AgentId::new())
            .build();
        assert!(cmd.validate().is_ok());
    }
}
```

**Integration tests** (`tests/` directory):
- Moderate speed: <5s per test
- Real services via wiremock or testcontainers
- Test the crate's public API as an external consumer would
- May use shared test utilities from `tests/common/mod.rs`

```rust
// ✓ Good: Integration test in tests/ directory
// tests/event_append.rs
use myapp_store::EventStore;

#[tokio::test]
async fn appends_and_replays_events() {
    let store = EventStore::in_memory().await.unwrap();
    store.append(test_event()).await.unwrap();
    let events = store.replay(stream_id).await.unwrap();
    assert_eq!(events.len(), 1);
}
```

**Doc tests** (`///` examples in public API):
- Minimal, focused on API usage
- Must compile and run
- Not a substitute for unit or integration tests

**Rules:**
- Unit tests live in `#[cfg(test)]` modules colocated with source (Rust convention)
- Integration tests live in `tests/` at the crate root
- Never place business logic in test helpers
- Mirror module structure in integration test file naming

**Why:** Rust's native test layout provides natural tier separation. Colocated unit tests keep test code close to the code it validates. Integration tests in `tests/` exercise the public API boundary, catching issues that unit tests miss.
