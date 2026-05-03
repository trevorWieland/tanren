---
kind: standard
name: mock-boundaries
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - rust
applies_to_domains:
  - testing
---

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

## Per-interface BDD wire harnesses (R-0001)

BDD scenarios route through `tanren_testkit::AccountHarness` (and
analogous harness traits introduced for future feature areas) — never
through direct calls to `tanren_app_services::Handlers` from step
definitions. The harness selected by the scenario's interface tag
(`@api`, `@cli`, `@mcp`, `@tui`, `@web`) is what earns the witness; an
in-process handler call cannot prove an `@api` scenario.

```rust
// ✓ Good: step dispatches through the per-interface harness
async fn given_signed_up(world: &mut TanrenWorld, email: String) {
    world.harness_mut().sign_up(SignUpRequest { email, .. }).await.unwrap();
}
```

```rust
// ✗ Bad: step calls the in-process handler directly, bypassing the wire
async fn given_signed_up(world: &mut TanrenWorld, email: String) {
    tanren_app_services::Handlers::sign_up(&world.deps, req).await.unwrap();
}
```

Mechanical enforcement:

- `xtask check-bdd-wire-coverage` (AST walker) rejects any direct
  `Handlers::` call inside `crates/tanren-bdd/src/steps/**`.
- `xtask check-deps` rejects `tanren-app-services` as a dependency of
  the `tanren-bdd` crate. The crate cannot reach the in-process surface
  even by accident.

Boundary mocking still applies to systems external to tanren: databases
are exercised against ephemeral test instances (e.g. SQLite tmpfile or
testcontainers), and outbound HTTP is faked with `wiremock`. The
prohibition is on bypassing tanren's own interface surfaces, not on
faking third parties.

For details of each per-interface harness implementation, see
`bdd-wire-harness.md`.

**Why:** Boundary-focused mocking keeps behavior scenarios stable under
refactor. Routing every interface witness through a real wire harness
keeps the proof honest — the tag describes what was actually driven.
