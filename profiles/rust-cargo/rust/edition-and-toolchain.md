---
kind: standard
name: edition-and-toolchain
category: rust
importance: high
applies_to:
  - "**/*.rs"
applies_to_languages:
  - rust
applies_to_domains:
  - rust
---

# Edition and Toolchain

Edition 2024, MSRV 1.85, stable channel. Toolchain pinned via `rust-toolchain.toml` checked into the repo.

```toml
# ✓ Good: rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "llvm-tools-preview"]
```

```toml
# ✗ Bad: No toolchain file, or using nightly
[toolchain]
channel = "nightly"   # Unstable features break without warning
```

**Required config files:**

```toml
# rustfmt.toml — use defaults, only set edition
edition = "2024"
```

```toml
# clippy.toml — enforce function line limit
too-many-lines-threshold = 100
# doc-valid-idents is project-specific:
# doc-valid-idents = ["SeaORM", "SQLite", "PostgreSQL", "UUIDv7"]
```

```toml
# taplo.toml — TOML formatter
[formatting]
align_entries = false
array_auto_expand = true
array_auto_collapse = true
```

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[alias]
t = "nextest run"
c = "clippy --workspace --all-targets"

[net]
retry = 3
```

**Workspace package defaults:**

```toml
# Root Cargo.toml
[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
```

**Notes:**
- Mold linker on Linux provides significant link-time speedup; macOS uses default linker (already fast)
- `llvm-tools-preview` component is required for `cargo-llvm-cov` coverage
- `doc-valid-idents` in `clippy.toml` is project-specific — add domain acronyms as needed
- `taplo fmt --check` uses an explicit glob list in CI/hooks to avoid missing TOML files

**Why:** A pinned toolchain ensures every developer and CI runner uses the same compiler version, eliminating "works on my machine" issues. Edition 2024 enables the latest language features while MSRV 1.85 ensures broad compatibility.
