---
kind: standard
name: dependency-management
category: global
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains: []
---

# Dependency Management

All dependencies pinned in workspace `[workspace.dependencies]`. Crates reference with `dep.workspace = true`. No version declarations in member crates.

```toml
# ✓ Good: Centralized workspace dependency
# Root Cargo.toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
thiserror = "2"

# Crate Cargo.toml
[dependencies]
serde = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
```

```toml
# ✗ Bad: Version declared in member crate
[dependencies]
serde = { version = "1.0.210", features = ["derive"] }
tokio = "1.40"
```

**Per-crate Cargo.toml pattern:**

```toml
[package]
name = "myapp-core"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lints]
workspace = true
```

**License policy (cargo-deny):**
- Allowed: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Unicode-DFS-2016, Zlib, BSL-1.0, CC0-1.0, CDLA-Permissive-2.0, OpenSSL
- No copyleft licenses (GPL, LGPL, AGPL, MPL-2.0)
- `confidence-threshold = 0.8`

**Source restrictions:**
- `unknown-registry = "deny"` — crates.io only
- `unknown-git = "deny"` — no git dependencies
- `wildcards = "deny"` — no wildcard version specs
- `allow-wildcard-paths = true` — workspace path deps parse as wildcard; this is expected

**Advisory policy:**
- Zero ignores for production dependencies
- Dev-only transitive ignores permitted with documented justification in `deny.toml`
- Security advisories addressed within 48 hours

**Unused dependency detection:**
- `cargo-machete` runs in CI to detect unused dependencies
- Remove unused deps immediately — don't leave dead weight

**Update strategy:**
- Update dependencies proactively with `cargo update`
- Review changelogs before major version bumps
- Run `just ci` after every dependency update

## Modern 2026 stack additions for R-0001

The R-0001 account-foundation slice introduces the following workspace
dependencies. Every entry below MUST land in `[workspace.dependencies]`
with the version pinned, and every member crate that uses one consumes
it via `{ workspace = true }` — never with an inline version.

| Crate | Pinned | Purpose |
|---|---|---|
| `argon2` | `0.5` | Password hashing (RustCrypto) |
| `password-hash` | `0.5` | PHC string format produced/verified by `argon2` |
| `tower-sessions` | `0.14` | HTTP session middleware for `tanren-api-app` |
| `tower-sessions-sqlx-store` | `0.14` | DB-backed session store using sqlx |
| `utoipa` | `5` | OpenAPI schema generation from types |
| `utoipa-axum` | `0.2` | Axum integration for `utoipa` route emission |
| `rand` | `0.9` | CSPRNG for opaque tokens (invitations, sessions) |
| `base64` | `0.22` | URL-safe encoding of opaque token bytes |
| `expectrl` | `0.7` | TUI test driver (PTY interaction) |
| `portable-pty` | `0.8` | Cross-platform PTY allocation under `expectrl` |
| `syn` | `2` | AST walker used by `xtask` enforcement checks |

Reaffirmed rule: every new dependency is added to
`[workspace.dependencies]` with a pinned version. Per-crate `Cargo.toml`
uses `{ workspace = true }`. Inline version specifications in member
crates are rejected by `cargo-deny` configuration and review.

**Why:** Centralized dependency management prevents version conflicts, ensures consistent feature flags, and makes auditing tractable. Strict license and source policies protect against legal and supply-chain risks.
