---
kind: standard
name: id-formats
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# ID Formats

Use UUIDv7 for all internal identifiers. Wrap in domain newtypes to prevent cross-type confusion. Store as TEXT in SQLite, UUID in PostgreSQL.

```rust
// ✓ Good: Domain ID newtype
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::fmt;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct DispatchId(Uuid);

impl DispatchId {
    /// Create a new time-ordered ID.
    #[must_use]
    pub fn fresh() -> Self {
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
- Derive `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema`
- Use `#[serde(transparent)]` for clean JSON serialization
- Implement `Display`, `From<Uuid>`, `AsRef<Uuid>`
- Provide a `fresh()` constructor returning `Uuid::now_v7()` (the canonical
  spelling; do not call it `new()` because `new` invites callers to assume
  side-effect-free construction)
- Never use auto-increment integers for primary keys

## Variable-length newtype recipe

Not every identifier is a UUID. `Email`, `Identifier` (a human-readable slug
or handle), and `InvitationToken` wrap a `String` (or `SecretString` for
token-shaped material) and validate at the construction boundary. They use
the same `JsonSchema`-augmented derive set, but expose `parse(&str)` /
`as_str()` instead of `fresh()` / `From<Uuid>`.

```rust
// ✓ Good: validated string newtype
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Email(String);

impl Email {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        // RFC 5322 / lite check, normalize case, ...
        Ok(Self(raw.trim().to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Identifier(String);

impl Identifier {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> { /* slug rules */ }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

```rust
// ✓ Good: invitation tokens are secret material
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvitationToken(SecretString);

impl InvitationToken {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> { /* base64url check */ }
    pub fn as_str(&self) -> &str { self.0.expose_secret() }
}
```

## Opaque secret tokens are not UUIDs

Session tokens, API keys, invitation tokens, and other bearer-shaped
material are **not** UUIDs. They are 256 bits of cryptographic randomness
(`rand::random::<[u8; 32]>()`), encoded base64url-no-pad, and wrapped in a
secret-bearing newtype:

```rust
// ✓ Good: opaque session token
pub struct SessionToken(SecretString);

impl SessionToken {
    pub fn fresh() -> Self {
        let bytes: [u8; 32] = rand::random();
        let s = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        Self(SecretString::new(s))
    }
}
```

```rust
// ✗ Bad: leaking entropy / shape by reusing UUIDs as session tokens
pub struct SessionToken(Uuid);   // 122 bits — too narrow, and structured
```

UUIDv7s carry timestamp bits in their high half, which is the right thing
for record IDs and the wrong thing for credentials — opaque tokens should
not encode their issuance time.

## Mechanical enforcement: `xtask check-newtype-ids`

An AST walker (run via `just check-newtype-ids`) inspects struct fields
declared in the contract / store / policy / app-services crates and rejects
bare `uuid::Uuid` field types. Every UUID-typed field must be wrapped in a
newtype defined in `tanren-contract`, unless the field appears in
`xtask/uuid-allowlist.toml` (used for raw migration plumbing only). The
walker runs across:

- `tanren-contract`
- `tanren-store`
- `tanren-identity-policy`
- `tanren-app-services`

In addition, the workspace `clippy.toml` carries

```toml
disallowed_methods = [
    { path = "uuid::Uuid::new_v4", reason = "use Uuid::now_v7 via the appropriate ID newtype" },
]
```

with a per-crate override only inside `tanren-store` (where migration tests
need to fabricate v4-shaped IDs to exercise legacy column tolerance).

**Why:** Newtypes make ID misuse a compile error rather than a runtime bug. UUIDv7 combines time-ordering (efficient indexing, natural sort) with global uniqueness (no sequence coordination). Distinguishing record IDs (UUIDv7 newtypes with `fresh()`) from variable-length identifiers (`Email`, `Identifier` with `parse()`) and from opaque bearer tokens (`SessionToken`, `InvitationToken` over 256-bit randomness) keeps each kind of identifier on the right entropy and lifecycle path. The type system enforces correct usage across crate boundaries.
