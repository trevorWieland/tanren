# No Test Skipping

Never use `#[ignore]` on tests. Tests either run and pass or don't exist yet. Zero skips allowed.

```rust
// ✓ Good: Test that runs and asserts
#[test]
fn parses_valid_config() {
    let config = Config::parse(VALID_TOML).unwrap();
    assert_eq!(config.max_retries, 3);
}
```

```rust
// ✗ Bad: Ignored test
#[test]
#[ignore]  // "too slow" — fix it or delete it
fn test_full_pipeline() { /* ... */ }
```

```rust
// ✗ Bad: Conditional skip
#[test]
fn test_with_postgres() {
    if std::env::var("DATABASE_URL").is_err() {
        return;  // Silent skip — hides untested code
    }
    // ...
}
```

```rust
// ✗ Bad: Feature-gated test exclusion
#[cfg(feature = "expensive-tests")]
#[test]
fn test_expensive_operation() { /* ... */ }
```

**Rules:**
- No `#[ignore]` attribute on any test
- No early `return` based on environment detection
- No feature-gated test exclusion (use separate test binaries or nextest filters instead)
- Flaky tests: make deterministic or delete — never ignore
- Slow tests: optimize or move to integration tier — never skip

**Handling external dependencies:**
- Database tests: use in-memory SQLite by default, gate Postgres behind a feature + separate CI job
- HTTP tests: use `wiremock` for local mock servers
- Time-dependent tests: inject a `Clock` trait, mock in tests

**Enforcement:**
- `just check-suppression` greps source directories for `#[ignore]`
- CI quality-gates job fails on any `#[ignore]` occurrence
- Code review catches conditional skips

**Why:** Skipped tests are invisible failures. They accumulate silently, giving false confidence in test coverage. A test that doesn't run provides zero value and should either be fixed or removed.
