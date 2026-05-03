---
kind: standard
name: type-safety-patterns
category: rust
importance: high
applies_to:
  - "**/*.rs"
applies_to_languages:
  - rust
applies_to_domains:
  - rust
---

# Type Safety Patterns

Use newtypes for domain IDs. Use `uuid::Uuid` v7 for time-ordered identifiers. Leverage the type system to encode invariants at compile time.

```rust
// ✓ Good: Domain ID newtype
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Uuid);

impl UserId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for UserId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl AsRef<Uuid> for UserId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}
```

```rust
// ✗ Bad: Raw types for domain concepts
fn assign_task(user_id: String, task_id: String) { /* easy to swap args */ }
fn set_amount(cents: i64) { /* is this cents? dollars? */ }
```

**Enums over strings:**

```rust
// ✓ Good: Enum for fixed value sets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

// ✗ Bad: String for fixed values
pub status: String  // "pending" | "running" — typo-prone, no exhaustiveness check
```

**Invariant encoding:**

```rust
// ✓ Good: Type-level invariants
use std::num::NonZeroU32;

pub struct RetryConfig {
    pub max_retries: NonZeroU32,  // Cannot be zero by construction
    pub timeout: Duration,
}
```

**Rules:**
- All domain IDs: newtype wrapping `Uuid`, created via `Uuid::now_v7()`
- Derive `Debug, Clone, Copy, PartialEq, Eq, Hash` on ID newtypes
- Implement `Display`, `From<Uuid>`, `AsRef<Uuid>`, `Serialize`/`Deserialize`
- Use enums (not strings) for any value with a fixed set of variants
- Use `NonZero*`, `NonEmpty<Vec<T>>`, and similar types to encode invariants
- Prefer `#[serde(rename_all = "snake_case")]` on enums for consistent serialization

## Cross-link: ID newtype enforcement

ID newtype formatting and the canonical Display/serialization contract
live in the `id-formats.md` standard. Mechanical enforcement in tanren
is done by an AST walker:

- `xtask check-newtype-ids` rejects bare `uuid::Uuid` field types inside
  `tanren-{contract,store,identity-policy,app-services}` crates outside
  the newtype declaration sites themselves. New domain IDs MUST land as
  newtypes; raw `Uuid` in those crates is a hard fail at the gate.
- The workspace `clippy.toml` denies `uuid::Uuid::new_v4` in
  handler/binary crates so opaque tokens are derived from CSPRNG bytes
  (`rand`) rather than v4 UUIDs. UUIDv7 remains the choice for domain
  IDs created via the newtype constructors.

See `id-formats.md` for the full Display/PHC-style/serialization
contract these newtypes implement.

**Why:** Newtypes prevent cross-type confusion at compile time (can't pass `UserId` where `OrderId` is expected). UUIDv7 provides time-ordered, globally-unique identifiers without coordination. Type-level invariants eliminate runtime validation for structurally impossible states.
