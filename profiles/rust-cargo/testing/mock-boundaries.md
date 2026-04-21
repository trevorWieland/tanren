# Mock Boundaries in BDD Scenarios

In Rust BDD suites, mock only trait-based external boundaries while keeping observable behavior realistic.

```rust
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> { self.0 }
}
```

```gherkin
@behavior(BEH-SCHED-003) @tier(unit)
Scenario: Scheduler picks next hourly window
  Given a fixed clock at "2026-01-15T09:00:00Z"
  When scheduling runs
  Then the next window is "2026-01-15T10:00:00Z"
```

**Rules:**
- Mock at trait boundaries only
- Do not mock private helper internals
- Scenario steps must assert observable outcomes, not internal calls
- Prefer wiremock/testcontainers for integration behavior

**Why:** Boundary-focused mocking keeps behavior scenarios stable under refactor.
