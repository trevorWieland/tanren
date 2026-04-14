# Test Timing Rules

Enforce timing limits per tier. Unit tests under 250ms with no I/O. Integration tests under 5 seconds. Nextest slow-timeout catches violators.

```rust
// ✓ Good: Fast unit test — pure logic, no I/O
#[test]
fn validates_cron_expression() {
    let result = CronExpr::parse("0 9 * * 1-5");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().next_fire().weekday(), Weekday::Mon);
}
// Runs in <1ms
```

```rust
// ✗ Bad: Unit test with hidden I/O
#[test]
fn loads_config_from_disk() {
    let config = Config::from_file("test_config.toml");  // File I/O in unit test
    assert!(config.is_ok());
}
// Slow, flaky, depends on filesystem state
```

```rust
// ✓ Good: Integration test with real service, bounded time
#[tokio::test]
async fn appends_event_to_store() {
    let store = EventStore::in_memory().await.unwrap();
    store.append(test_event()).await.unwrap();
    let events = store.replay_all().await.unwrap();
    assert_eq!(events.len(), 1);
}
// Runs in ~50ms with in-memory SQLite
```

**Timing limits:**

| Tier | Max Time | I/O Allowed | Network Allowed |
|------|----------|-------------|-----------------|
| Unit | 250ms | No | No |
| Integration | 5s | Yes | Yes (wiremock/testcontainers) |
| Doc tests | 250ms | No | No |

**Nextest enforcement:**
- Default profile: `slow-timeout = { period = "60s", terminate-after = 2 }` — flags slow tests, kills after 2 periods
- CI profile: `slow-timeout = { period = "120s", terminate-after = 3 }` — more lenient for CI runners

**Refactoring slow tests:**
- Extract I/O behind a trait, mock in unit tests
- Use in-memory databases instead of file-backed
- Reduce test data size while maintaining coverage
- Parallelize independent assertions

**Why:** Fast tests enable tight feedback loops. A test suite that runs in seconds gets run frequently; one that takes minutes gets skipped. Timing limits prevent gradual test suite slowdown that erodes developer productivity.
