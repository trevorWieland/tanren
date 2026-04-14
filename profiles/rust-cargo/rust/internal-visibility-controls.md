# Internal Visibility Controls

Every access restriction must be enforced by the compiler. If rustc doesn't reject the misuse, the restriction doesn't exist. Never rely on documentation, naming conventions, or social agreement to prevent incorrect access to internal methods.

## Guiding Principle

The compiler is the reviewer that never sleeps. A method that is `pub` **will** be called by a downstream crate eventually — regardless of doc comments, `#[doc(hidden)]`, `__` prefixes, or "do not use" warnings. The only mechanisms that actually prevent misuse are:

1. **Rust's visibility system** (`pub(crate)`, private modules, selective re-exports)
2. **Conditional compilation** (`#[cfg(feature = "...")]` with `required-features`)

Everything else is a suggestion, not an enforcement.

## Decision Tree

When a method should not be part of the public API, follow this priority strictly:

### 1. Redesign the public API (strongest)

Ask: why does the test need internal access? Often the answer is that the public API is incomplete. A facade that delegates to `pub(crate)` operations while exposing a complete set of public methods eliminates the need for escape hatches entirely.

```rust
// ✓ Good: Facade with complete public API
// No internal access needed — tests use the same API as production code
pub struct Store { /* private fields */ }

impl Store {
    pub async fn append_message(&self, msg: &Message) -> Result<()> {
        ops::messages::append(&self.db, msg).await  // pub(crate) delegation
    }
    pub async fn replay_stream(&self, id: StreamId) -> Result<Vec<Event>> {
        ops::messages::replay(&self.db, id).await
    }
}
```

```rust
// ✗ Bad: Incomplete public API that forces tests to reach inside
pub struct Store { pub db: DatabaseConnection }  // Exposed internals
```

If the public API is sufficient, integration tests need nothing extra. This is the gold standard.

### 2. `pub(crate)` + unit tests (strong)

If the method is truly internal — called only within the crate — make it `pub(crate)`. Test it from `#[cfg(test)] mod tests` inside the crate, where `pub(crate)` methods are visible.

```rust
// ✓ Good: Internal method with crate-only visibility
pub(crate) async fn dequeue_impl(db: &DatabaseConnection, lane: Lane) -> Result<Job> {
    // Only callable within this crate — compiler enforces it
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dequeues_from_correct_lane() {
        // Can call dequeue_impl here — same crate
    }
}
```

```rust
// ✗ Bad: pub method with a "please don't call this" comment
/// Internal — do not call from outside this crate.
pub async fn dequeue_impl(/* ... */) { /* ... */ }  // Nothing stops them
```

Key constraint: integration tests in `tests/` are **external crate roots** — they cannot see `pub(crate)` methods. If the method must be tested from `tests/`, you need option 3.

### 3. Feature-gated `pub` + `required-features` (pragmatic)

When integration tests genuinely need internal access — test fixture seeding, raw data insertion, schema introspection, invariant-bypassing helpers — use a Cargo feature gate with three mandatory components:

1. `#[cfg(feature = "test-hooks")]` on the impl block
2. `[[test]]` entries with `required-features = ["test-hooks"]` in Cargo.toml
3. `--features crate-name/test-hooks` in **every** justfile recipe and CI job that compiles test targets

All three are required. Missing any one creates a gap.

### 4. Never: `#[doc(hidden)]`, naming conventions, or comments

These are not visibility mechanisms:

| Approach | What it actually does | Prevents misuse? |
|----------|----------------------|-----------------|
| `#[doc(hidden)]` | Hides from `cargo doc` output | **No** — any crate can still call it |
| `__` prefix | Signals "private" by convention | **No** — compiles and runs normally |
| Doc comment "do not use" | Communicates intent to readers | **No** — has zero compile-time effect |
| Renaming to `_unchecked` | Signals danger | **No** — still callable |

The sole acceptable use of `#[doc(hidden)]` is satisfying lint requirements where macro-generated code forces `pub` visibility but the containing module is unexported:

```rust
// Acceptable: SeaORM entities need pub but module is private
// lib.rs
#[doc(hidden)]
pub mod entity;  // pub for E0446, module unexported, types unreachable
```

## Feature Gate Pattern — Complete Implementation

### Cargo.toml

```toml
[features]
default = []
# Exposes Store::append / Store::append_batch for integration tests.
# Production code must NOT enable this — bypasses projection consistency.
# Test binaries declare required-features so bare `cargo test` silently
# skips them rather than failing to compile.
test-hooks = []

# Every test binary that calls gated methods MUST have an entry here.
# Auto-discovered tests (no [[test]] entry) will FAIL TO COMPILE
# without the feature — this is the single most common mistake.
[[test]]
name = "sqlite_integration"
path = "tests/sqlite_integration.rs"
required-features = ["test-hooks"]

[[test]]
name = "event_query"
path = "tests/event_query.rs"
required-features = ["test-hooks"]

# Features can compose — postgres tests need both hooks and postgres
[[test]]
name = "postgres_integration"
path = "tests/postgres_integration.rs"
required-features = ["test-hooks", "postgres-integration"]
```

### Source code

```rust
/// Test-only escape hatches — gated behind the `test-hooks` feature.
/// These methods are conditionally compiled out of production builds.
#[cfg(feature = "test-hooks")]
impl Store {
    /// Append a single event. Bypasses projection consistency.
    pub async fn append(&self, event: &EventEnvelope) -> Result<()> {
        // Use fully-qualified paths for types only used in gated blocks
        // to avoid unused-import warnings when the feature is off.
        let model = tanren_domain::EventEnvelope::to_active_model(event);
        // ...
    }
}
```

### Justfile — every recipe that compiles test targets

```just
# Scope the feature per-crate, not workspace-wide.
# Workspace-wide --features test-hooks would silently activate a
# test-hooks feature on ANY crate that defines one — a latent
# cross-crate collision risk.

check:
    @{{ cargo }} check --workspace --all-targets --features myapp-store/test-hooks --quiet

test *args:
    @{{ cargo }} nextest run --workspace --features myapp-store/test-hooks {{ args }}

coverage:
    @{{ cargo }} llvm-cov nextest --workspace --features myapp-store/test-hooks --lcov --output-path lcov.info

lint:
    @{{ cargo }} clippy --workspace --all-targets --features myapp-store/test-hooks --quiet -- -D warnings

fix:
    @{{ cargo }} clippy --workspace --all-targets --features myapp-store/test-hooks --fix --allow-dirty --allow-staged --quiet -- -D warnings

# doc does NOT get the feature — gated methods should be invisible in public docs
doc:
    @RUSTDOCFLAGS="-D warnings" {{ cargo }} doc --workspace --no-deps --quiet
```

### CI — must mirror justfile exactly

```yaml
# Clippy must also pass the feature. Without it, clippy skips
# feature-gated test binaries (via required-features), meaning
# the test code is never linted. Unlinted test code accumulates
# warnings that only surface when someone runs clippy locally
# with the feature enabled.
- name: Run clippy
  run: cargo clippy --workspace --all-targets --features myapp-store/test-hooks -- -D warnings

- name: Run tests
  run: cargo nextest run --workspace --features myapp-store/test-hooks --profile ci

- name: Generate coverage
  run: cargo llvm-cov nextest --workspace --features myapp-store/test-hooks --lcov --output-path lcov.info
```

## Why `required-features` is non-negotiable

Without `required-features` on `[[test]]` entries:

```
$ cargo test                    # No --features flag
error[E0599]: no method named `append` found for struct `Store`
  --> tests/event_query.rs:42:11
```

With `required-features`:

```
$ cargo test                    # No --features flag
# test binary "event_query" silently skipped — feature not enabled
# All other tests compile and run normally
```

The difference: a hard compile error vs. graceful degradation. Any developer or CI job that runs bare `cargo test` hits an incomprehensible error without `required-features`. With it, the gated tests simply don't run — they require an explicit opt-in via `--features`.

## Module-level encapsulation patterns

Beyond method visibility, use module structure to minimize the public surface:

```rust
// ✓ Good: Private module with selective re-exports
// lib.rs
mod ops;           // private — nothing leaks
mod entities;      // private — SeaORM models stay internal
mod migrations;    // private — migration details stay internal

pub use store::{Store, StoreBackend};     // facade only
pub use error::{StoreError, RetryHint};   // error types
pub use types::{ReplayCursor};            // value types
```

```rust
// ✗ Bad: Public modules exposing internals
pub mod ops;       // Every operation function is now public API
pub mod entities;  // Every database model is now public API
```

**Rules:**
- **Compiler enforcement only** — if rustc doesn't reject it, it's not restricted
- **Feature name:** `test-hooks` (consistent across all projects in the workspace)
- **Every test binary** using gated methods must have a `[[test]]` entry with `required-features`
- **Justfile and CI must match** — every recipe and job that compiles test targets passes the feature, including clippy
- **Scope per-crate:** `--features myapp-store/test-hooks` (never workspace-wide)
- **`doc` excluded:** gated methods must not appear in public documentation
- **`pub(crate)` is always preferred** over feature gates when the test can live inside the crate
- **`#[doc(hidden)]` is never a visibility mechanism** — only acceptable for lint satisfaction on unexported modules

**Why:** The cost of a compile-time error for misuse is zero — it's caught before the code runs, before the PR is opened, before the reviewer sees it. The cost of a runtime bug from calling an internal method that bypasses safety invariants is unbounded. Every access restriction that isn't compiler-enforced will eventually be violated.
