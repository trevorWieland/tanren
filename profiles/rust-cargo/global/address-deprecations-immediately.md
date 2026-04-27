---
kind: standard
name: address-deprecations-immediately
category: global
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains: []
---

# Address Deprecations Immediately

Compiler warnings are errors. Never defer deprecation fixes. Never suppress with `#[allow(deprecated)]`.

```rust
// ✓ Good: Migrate to replacement API immediately
use std::sync::LazyLock;

static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    Config::from_env()
});
```

```rust
// ✗ Bad: Suppressing deprecation warning
#[allow(deprecated)]
use std::sync::lazy::SyncLazy; // Deprecated — use LazyLock
```

```rust
// ✗ Bad: Ignoring compiler warnings
// warning: use of deprecated function `old_api::connect`
// --> src/db.rs:42:5
// ... (left unfixed, "will get to it later")
```

**Rules:**
- `RUSTFLAGS="-D warnings"` in CI — all warnings are errors
- Clippy warnings are errors: `cargo clippy -- -D warnings`
- Doc warnings are errors: `RUSTDOCFLAGS="-D warnings"`
- When a dependency deprecates an API, migrate immediately in the same PR that updates the dep
- When the Rust compiler deprecates a feature, migrate before the next toolchain bump

**Toolchain updates:**
- Update `rust-toolchain.toml` channel and MSRV proactively
- Review Rust release notes for deprecations and new features
- Migrate to new edition features when updating edition

**Exceptions:**
- Only when blocked on an upstream crate release that hasn't published the replacement yet
- Track with a GitHub issue linking to the upstream issue
- Never suppress — leave the warning visible as a reminder

**Why:** Deferred deprecations accumulate into migration debt that compounds with each release. Addressing them immediately keeps the codebase on the latest stable APIs, reduces surprise breakage during toolchain upgrades, and ensures CI stays green without warning suppression.
