# Mandatory Coverage

Coverage is mandatory. Use `cargo-llvm-cov` for measurement. Tests must exercise behavior, not just construction.

```bash
# ✓ Good: Generate coverage report
just coverage   # Runs: cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
```

```rust
// ✓ Good: Test that validates behavior
#[test]
fn rejects_expired_token() {
    let token = Token::new(Utc::now() - Duration::hours(1));
    let result = validate_token(&token);
    assert!(matches!(result, Err(AuthError::TokenExpired { .. })));
}
```

```rust
// ✗ Bad: Test that only checks construction
#[test]
fn creates_config() {
    let config = Config::default();
    assert!(config.is_ok());  // Doesn't test any behavior
}
```

**Rules:**
- `cargo llvm-cov nextest` for coverage measurement (combines nextest with LLVM instrumentation)
- Output as lcov for CI integration: `--lcov --output-path lcov.info`
- Upload to Codecov (or equivalent) in CI for tracking trends
- Tests must exercise:
  - Happy path with expected output validation
  - Error cases with specific error variant checks
  - Edge cases (empty input, boundary values, concurrent access)
- Coverage is a signal, not a target — 100% line coverage with shallow assertions is worse than 80% with deep behavioral tests

**CI pipeline:**

```yaml
# GitHub Actions coverage job
- name: Coverage
  run: cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
- name: Upload
  uses: codecov/codecov-action@v4
  with:
    files: lcov.info
```

**Why:** Coverage measurement identifies untested code paths before they become production bugs. Behavioral testing (vs construction testing) ensures tests catch regressions in actual logic, not just compilation.
