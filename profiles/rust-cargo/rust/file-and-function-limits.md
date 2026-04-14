# File and Function Limits

Maximum 500 lines per `.rs` file. Maximum 100 lines per function. No exceptions.

```bash
# ✓ Good: Well-structured module
$ wc -l crates/myapp-core/src/dispatch.rs
  187 crates/myapp-core/src/dispatch.rs    # Under 500

# ✗ Bad: Monolithic file
$ wc -l crates/myapp-core/src/engine.rs
  823 crates/myapp-core/src/engine.rs      # Over 500 — split it
```

```rust
// ✓ Good: Focused function under 100 lines
fn validate_dispatch(cmd: &DispatchCommand) -> Result<ValidatedDispatch, ValidationError> {
    let agent = resolve_agent(&cmd.agent_id)?;
    let capabilities = check_capabilities(&agent, &cmd.requirements)?;
    let budget = verify_budget(&cmd.budget_id, cmd.estimated_cost)?;
    Ok(ValidatedDispatch { agent, capabilities, budget })
}
```

```rust
// ✗ Bad: 200-line function doing too many things
fn process_everything(input: Input) -> Result<Output> {
    // ... 200 lines of mixed validation, processing, formatting, logging ...
}
```

**Refactoring strategies when hitting limits:**

**File too long (>500 lines):**
- Split into submodules: `dispatch/mod.rs`, `dispatch/validate.rs`, `dispatch/execute.rs`
- Extract types into a separate `types.rs` or `models.rs` module
- Move `#[cfg(test)]` module to a separate test file if it's large

**Function too long (>100 lines):**
- Extract helper functions with descriptive names
- Use early returns to reduce nesting
- Break into a pipeline of small transforms

**Enforcement:**
- `just check-lines` recipe scans all `.rs` files in source directories, fails if any exceed 500 lines
- `clippy.toml` sets `too-many-lines-threshold = 100`, enforced by clippy `too_many_lines` lint
- Both run in CI via the quality-gates job

**Why:** Short files and functions are easier to navigate, review, and test. The 500/100 limits force modular design without being so restrictive that they cause artificial splitting. These limits catch organic growth before files become unwieldy.
