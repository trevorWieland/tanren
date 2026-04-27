---
kind: standard
name: naming-conventions
category: rust
importance: high
applies_to:
  - "**/*.rs"
applies_to_languages:
  - rust
applies_to_domains:
  - rust
---

# Naming Conventions

Follow Rust naming conventions strictly. Use Conventional Commits with scope for all commit messages.

```rust
// ✓ Good: Rust naming standards
mod task_scheduler;              // Module: snake_case
pub struct TaskScheduler;        // Type: PascalCase
pub trait Dispatchable;          // Trait: PascalCase
pub fn schedule_task();          // Function: snake_case, verb-first
const MAX_RETRY_COUNT: u32 = 5;  // Constant: SCREAMING_SNAKE_CASE
let active_tasks = vec![];       // Variable: snake_case
```

```toml
# ✓ Good: Crate naming in Cargo.toml
[package]
name = "myapp-scheduler"   # kebab-case in Cargo.toml
```

```rust
// In Rust source, the crate name becomes snake_case:
use myapp_scheduler::TaskScheduler;
```

```rust
// ✗ Bad: Inconsistent naming
pub struct taskScheduler;   // Types must be PascalCase
pub fn ScheduleTask();      // Functions must be snake_case
mod TaskScheduler;          // Modules must be snake_case
```

**Error type naming:**

```rust
// ✓ Good: {Domain}Error pattern
pub enum AuthError { /* ... */ }
pub enum StorageError { /* ... */ }
pub enum SchedulerError { /* ... */ }

// ✗ Bad: Generic or inconsistent
pub enum Error { /* ... */ }       // Too generic for a library crate
pub enum AuthFailure { /* ... */ } // Inconsistent suffix
```

**Feature flags:**

```toml
# ✓ Good: kebab-case features
[features]
postgres-integration = []
test-hooks = []
```

**Conventional Commits:**

```
feat(scheduler): add priority-based lane selection
fix(store): close TOCTOU race in dequeue
refactor(domain): extract dispatch state machine
test(policy): add budget exhaustion property tests
docs(api): update OpenAPI schema for v2 endpoints
chore: update workspace dependencies
```

**Rules:**
- Commit messages must use Conventional Commits format: `type(scope): description`
- Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`, `ci`
- Scope matches the crate name without the project prefix (e.g., `store` not `myapp-store`)
- Description is lowercase, imperative mood, no period

**Why:** Consistent naming reduces cognitive load and makes the codebase navigable. Conventional Commits enable automated changelogs and make git history scannable by domain area.
