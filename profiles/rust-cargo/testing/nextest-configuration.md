# Nextest Configuration

Use `cargo-nextest` for all test execution. Never use `cargo test`. Configure default and CI profiles with timeouts and retry policies.

```bash
# ✓ Good: Run tests with nextest
cargo nextest run              # Or: cargo t (alias)
just test                      # Wrapper that uses nextest
just test -p myapp-core        # Single crate

# ✗ Bad: Using cargo test
cargo test                     # Missing: parallel control, timeouts, output filtering
```

**Required `.config/nextest.toml`:**

```toml
[profile.default]
slow-timeout = { period = "60s", terminate-after = 2 }
failure-output = "immediate"
success-output = "never"
status-level = "slow"

[profile.ci]
fail-fast = false
failure-output = "immediate-final"
status-level = "all"
retries = 2
slow-timeout = { period = "120s", terminate-after = 3 }
```

**Test groups for serialized database tests:**

```toml
[test-groups]
postgres-integration = { max-threads = 1 }

[[profile.default.overrides]]
filter = "binary(postgres_integration)"
test-group = "postgres-integration"

[[profile.ci.overrides]]
filter = "binary(postgres_integration)"
test-group = "postgres-integration"
```

**Rules:**
- Default profile: 60s slow-timeout, immediate failure output, hide success output
- CI profile: 120s slow-timeout, 2 retries, no fail-fast (run all tests even after failure)
- Database integration tests: serialize with `max-threads = 1` to prevent schema race conditions
- SQLite tests remain parallel (separate database files per test)
- CI runs with `cargo nextest run --profile ci`

**Why:** Nextest provides parallel execution, structured output, timeout enforcement, and retry policies that `cargo test` lacks. Slow-timeout catches tests that hang or regress in performance. CI retries handle transient failures without masking real bugs.
