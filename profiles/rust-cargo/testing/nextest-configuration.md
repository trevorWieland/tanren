# Nextest Configuration

`cucumber-rs` scenarios are the authoritative behavior tests. `cargo-nextest` remains the required Rust test runner for applicable test binaries and support checks in the CI gate.

```bash
# Behavior scenarios (authoritative)
cargo cucumber

# Fast Rust test gate execution where applicable
cargo nextest run --workspace --profile ci
```

**Rules:**
- Behavior proof comes from `.feature` scenarios executed via `cucumber-rs`
- Nextest remains mandatory for workspace Rust test execution where applicable
- CI must run both behavior scenarios and nextest gate checks
- Do not replace scenario execution with direct non-BDD-only suites

**Why:** BDD scenarios prove behavior, while nextest preserves fast, structured Rust test execution discipline.
