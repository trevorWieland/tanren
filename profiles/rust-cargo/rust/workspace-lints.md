# Workspace Lints

All lints configured in workspace `[workspace.lints]` and inherited by every crate. No inline `#[allow]` or `#[expect]` attributes anywhere in source code.

```toml
# ✓ Good: Workspace-level lint configuration
# Root Cargo.toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
unused_qualifications = "warn"

[workspace.lints.clippy]
# Strict lint groups
correctness = { level = "deny", priority = -1 }
suspicious = { level = "deny", priority = -1 }
perf = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }

# Hard denies — these are never acceptable
unwrap_used = "deny"
panic = "deny"
todo = "deny"
dbg_macro = "deny"
print_stdout = "deny"
print_stderr = "deny"
unimplemented = "deny"

# Prohibit inline suppression
allow_attributes = "deny"
allow_attributes_without_reason = "deny"

# Pedantic overrides (too noisy for practical use)
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
```

```toml
# ✓ Good: Crate inherits workspace lints
# crates/myapp-core/Cargo.toml
[lints]
workspace = true
```

```rust
// ✗ Bad: Inline lint suppression
#[allow(clippy::too_many_arguments)]  // Denied by allow_attributes
fn process(a: i32, b: i32, c: i32, d: i32, e: i32) { /* ... */ }
```

**Relaxing a lint for a specific crate:**

When a crate legitimately needs a lint relaxed (e.g., macro-generated code), override in that crate's `Cargo.toml` with a comment:

```toml
# crates/myapp-storage/Cargo.toml
[lints.rust]
unsafe_code = "forbid"
# SeaORM DeriveEntityModel requires pub visibility on generated types
unreachable_pub = "allow"
```

**Enforcement:**
- `allow_attributes = "deny"` catches any inline `#[allow()]` in source
- `just check-suppression` scans for `#[allow(`, `#[expect(`, `#![allow(` across all source directories
- CI quality-gates job runs `check-suppression`

**Why:** Centralized lint configuration ensures consistent code quality across all crates. Prohibiting inline suppression forces developers to justify relaxations at the crate level where they're visible during review, preventing lint erosion one `#[allow]` at a time.
