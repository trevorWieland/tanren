---
kind: standard
name: error-handling
category: rust
importance: high
applies_to:
  - "**/*.rs"
applies_to_languages:
  - rust
applies_to_domains:
  - rust
---

# Error Handling

Use `thiserror` in library crates for typed, structured errors. Use `anyhow` only in binary crates for top-level error propagation. Never panic.

```rust
// ✓ Good: Library crate with thiserror
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("record not found: {id}")]
    NotFound { id: Uuid },
    #[error("duplicate key: {key}")]
    DuplicateKey { key: String },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub fn get_record(id: Uuid) -> Result<Record, StorageError> {
    // ...
}
```

```rust
// ✓ Good: Binary crate with anyhow
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = Config::load()
        .context("failed to load configuration")?;
    run_server(config).await
        .context("server exited with error")
}
```

```rust
// ✗ Bad: unwrap, expect, panic, todo
let value = map.get("key").unwrap();           // Denied
let conn = connect().expect("must connect");   // Denied
panic!("unexpected state");                     // Denied
todo!("implement later");                       // Denied
```

**Option handling:**

```rust
// ✓ Good: Convert Option to Result
let user = users.get(&id)
    .ok_or_else(|| AuthError::UserNotFound { id })?;

// ✓ Good: Pattern match
match config.optional_field {
    Some(value) => process(value),
    None => use_default(),
}
```

**Rules:**
- Library crates: define domain error enums with `#[derive(thiserror::Error)]`
- Binary crates: use `anyhow::Result` with `.context()` for human-readable error chains
- Never use `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, `unimplemented!()`
- Use `#[from]` for automatic error conversion between crate boundaries
- Prefer `?` over manual `match` on `Result` when the error type converts

**Why:** Typed errors in libraries give callers the ability to match on specific failure modes. `anyhow` in binaries provides rich error context for debugging. Banning panics ensures the program degrades gracefully rather than crashing.
