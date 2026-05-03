---
kind: standard
name: crate-layering
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# Crate Layering

Enforce dependency direction. Core never depends on infrastructure. Infrastructure never depends on API/CLI. Validate with `just check-deps`.

```
# ✓ Good: Correct dependency direction
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  bin/api     │────▶│  orchestrator│────▶│   domain     │
│  bin/cli     │     │  store       │     │   policy     │
└─────────────┘     └──────────────┘     └──────────────┘
   Binaries            Infrastructure        Foundation
   (anyhow)            (thiserror)           (thiserror)
```

```toml
# ✓ Good: Domain crate has zero infrastructure deps
# crates/myapp-domain/Cargo.toml
[dependencies]
serde = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
# No sqlx, no reqwest, no tokio — pure domain logic
```

```toml
# ✗ Bad: Domain importing infrastructure
# crates/myapp-domain/Cargo.toml
[dependencies]
sqlx = { workspace = true }    # Database driver in domain!
reqwest = { workspace = true } # HTTP client in domain!
```

**Layering rules:**

| Rule | Description |
|------|-------------|
| Core rule | Domain crate never imports from any other workspace crate |
| Storage rule | Only store crate owns SQL and query details |
| Transport rule | Binaries depend on app-services + contract (+ runtime for wiring) |
| Policy rule | Policy returns typed decisions, never transport-layer errors |
| Runtime rule | Runtime/harness crates never own policy decisions |
| Observability rule | No crate emits unstructured logs without correlation context |

**Automated enforcement with `check-deps`:**

```bash
# justfile recipe
check-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    foundation=("myapp-domain" "myapp-policy" "myapp-contract")
    capability=("myapp-orchestrator" "myapp-store")
    for cap in "${capability[@]}"; do
        deps=$(cargo metadata --format-version 1 \
            | jq -r ".packages[] | select(.name == \"$cap\") | .dependencies[].name")
        for found in "${foundation[@]}"; do
            # Foundation crates must not depend on capability crates
            # (reverse direction check)
        done
    done
    echo "check-deps: all layering rules pass"
```

**Rules:**
- Foundation crates: domain types, policies, contracts — zero I/O dependencies
- Capability crates: orchestration, storage, runtime — implement traits from foundation
- Binary crates: composition wiring — depend on capability + foundation
- Validate layering in CI via `just check-deps` in quality-gates job
- Add `check-deps` to the `ci` gate for all projects with 3+ crates

## Per-binary library crate pattern (R-0001)

Cargo binary crates cannot be depended on by other crates, including
integration tests. Any non-trivial logic that lives only in `bin/X/src/main.rs`
is therefore unreachable from the BDD harness, from doctests, and from sibling
binaries. R-0001 promotes every binary's logic into a sibling **library crate**:

```
crates/
  tanren-api-app/    # all API binary logic, exposes serve(config) -> Future
  tanren-cli-app/    # all CLI binary logic,  exposes run(config)   -> Future
  tanren-mcp-app/    # all MCP binary logic,  exposes serve(config) -> Future
  tanren-tui-app/    # all TUI binary logic,  exposes run(config)   -> Future

bin/
  tanren-api/src/main.rs   # ≤ 50 lines: parse → tracing → call serve()
  tanren-cli/src/main.rs   # ≤ 50 lines
  tanren-mcp/src/main.rs   # ≤ 50 lines
  tanren-tui/src/main.rs   # ≤ 50 lines
```

```rust
// ✓ Good: library crate owns the logic
// crates/tanren-api-app/src/lib.rs
pub async fn serve(config: ApiConfig) -> anyhow::Result<()> {
    let store = SeaOrmStore::connect(&config.database_url).await?;
    let router = build_router(store);
    axum::serve(listener, router).await
}
```

```rust
// ✓ Good: bin/ shell is wiring only
// bin/tanren-api/src/main.rs
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tanren_observability::init(cli.log_filter())?;
    tanren_api_app::serve(cli.into_config()).await
}
```

The BDD harness depends on `tanren-api-app` (the library crate) directly,
exercising the same `serve()` entrypoint that the production binary boots.
A `tests/` integration test attempting to depend on `bin/tanren-api` would
fail to compile — the library-crate split is what makes the binary itself
testable.

**Why:** Enforced layering prevents accidental coupling that makes crates untestable in isolation. A domain crate that imports `sqlx` can no longer be tested without a database. Promoting binary logic into per-binary library crates extends the same principle to the binary surface — the BDD harness drives the same code path the shipped binary runs, instead of a parallel test-only fork. Automated checks catch violations before code review.
