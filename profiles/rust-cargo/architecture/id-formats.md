# ID Formats

Use UUIDv7 for all internal identifiers. Wrap in domain newtypes to prevent cross-type confusion. Store as TEXT in SQLite, UUID in PostgreSQL.

```rust
// ✓ Good: Domain ID newtype
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DispatchId(Uuid);

impl DispatchId {
    /// Create a new time-ordered ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl fmt::Display for DispatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for DispatchId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl AsRef<Uuid> for DispatchId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}
```

```rust
// ✓ Good: Type safety prevents misuse
fn assign(dispatch_id: DispatchId, agent_id: AgentId) -> Result<()>;

// Compile error: can't pass AgentId where DispatchId is expected
assign(agent_id, dispatch_id);  // Error!
```

```rust
// ✗ Bad: Raw types
fn assign(dispatch_id: Uuid, agent_id: Uuid) -> Result<()>;
// Easy to swap arguments — compiles but wrong
assign(agent_id_value, dispatch_id_value);  // Compiles! Bug.
```

```rust
// ✗ Bad: String IDs
fn assign(dispatch_id: &str, agent_id: &str) -> Result<()>;
// No type safety, no format validation, easy to pass garbage
```

**UUIDv7 properties:**
- Time-ordered: IDs sort chronologically without a separate timestamp column
- Globally unique: no coordination needed across distributed systems
- 128-bit: sufficient entropy for any scale
- Created via `Uuid::now_v7()` (requires `uuid` crate with `v7` feature)

**Database storage:**

| Database | Column Type | Rationale |
|----------|-------------|-----------|
| SQLite | `TEXT` | No native UUID type; TEXT preserves human readability |
| PostgreSQL | `UUID` | Native type with indexing optimizations |

**Rules:**
- Every domain entity gets its own ID newtype: `UserId`, `TaskId`, `AgentId`, etc.
- All IDs created via `Uuid::now_v7()` — never v4 (random, not sortable)
- Derive `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`
- Use `#[serde(transparent)]` for clean JSON serialization
- Implement `Display`, `From<Uuid>`, `AsRef<Uuid>`
- Never use auto-increment integers for primary keys

**Why:** Newtypes make ID misuse a compile error rather than a runtime bug. UUIDv7 combines time-ordering (efficient indexing, natural sort) with global uniqueness (no sequence coordination). The type system enforces correct usage across crate boundaries.
