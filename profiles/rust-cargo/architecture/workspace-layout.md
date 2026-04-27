---
kind: standard
name: workspace-layout
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# Workspace Layout

Use a Cargo workspace with a flat crate directory. Name crates `{project}-{domain}`. Every crate inherits workspace configuration.

```
# ✓ Good: Flat workspace layout
Cargo.toml              # Workspace root
rust-toolchain.toml
justfile
deny.toml
clippy.toml
taplo.toml
.cargo/config.toml
.config/nextest.toml
lefthook.yml

crates/
  myapp-domain/         # Core entities, no external deps
  myapp-store/          # Database adapters
  myapp-policy/         # Authorization, budgets
  myapp-orchestrator/   # Business logic engine

bin/
  myapp-api/            # HTTP server binary
  myapp-cli/            # CLI binary
```

```toml
# ✓ Good: Workspace root Cargo.toml
[workspace]
resolver = "2"
members = [
    "bin/myapp-api",
    "bin/myapp-cli",
    "crates/myapp-domain",
    "crates/myapp-store",
    "crates/myapp-policy",
    "crates/myapp-orchestrator",
]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# All dependencies centralized here

[workspace.lints.rust]
# All lints centralized here
```

```toml
# ✗ Bad: No workspace, standalone crates
# Each crate has its own version, edition, lint config
# Dependencies duplicated across Cargo.toml files
```

**Per-crate Cargo.toml template:**

```toml
[package]
name = "myapp-domain"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "Core domain entities and business rules"

[lints]
workspace = true

[dependencies]
serde = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
proptest = { workspace = true }
```

**Rules:**
- Library crates in `crates/`, binary crates in `bin/` (or `apps/`)
- All crates inherit workspace package config and lints
- `publish = false` unless publishing to crates.io
- Resolver `"2"` required for workspace dependencies feature
- Source directories searched by `check-lines` and `check-suppression` must include both `crates/` and `bin/` (or `apps/`)

**Why:** A workspace provides unified dependency management, shared lint configuration, and single-command builds. Flat crate layout keeps the project navigable as it grows.
