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

**Why:** Enforced layering prevents accidental coupling that makes crates untestable in isolation. A domain crate that imports `sqlx` can no longer be tested without a database. Automated checks catch violations before code review.
