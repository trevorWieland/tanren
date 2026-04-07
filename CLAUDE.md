# Tanren 2.0 — Project Conventions

> This file covers the Rust rewrite (`bin/`, `crates/`). For the legacy Python
> codebase (`packages/`, `services/`, `tests/`), see `AGENTS.md`.

## Architecture

Tanren is an agent orchestration control plane for software delivery. Cargo
workspace with 16 library crates and 5 binary crates. See
`docs/rewrite/CRATE_GUIDE.md` for the full topology and dependency graph.

## Workspace Structure

```
bin/
  tanrend              # daemon/control-plane runtime
  tanren-api           # HTTP API (axum)
  tanren-cli           # CLI (clap)
  tanren-mcp           # MCP server
  tanren-tui           # terminal UI (ratatui)

crates/
  tanren-domain        # canonical entities, commands, events, errors (no deps)
  tanren-contract      # interface schema generation and versioning
  tanren-policy        # authz, budgets, quotas, placement policy
  tanren-store         # event log, projections, migrations (sqlx)
  tanren-planner       # task graph planning and replanning
  tanren-scheduler     # lane/capability-aware dispatch scheduling
  tanren-runtime       # harness + environment trait contracts
  tanren-runtime-*     # concrete environment implementations
  tanren-harness-*     # concrete harness implementations
  tanren-orchestrator  # control-plane orchestration engine
  tanren-observability # tracing, metrics, correlation helpers
  tanren-app-services  # shared service layer for all interfaces
```

## Rust Conventions

- **Edition**: 2024, stable channel
- **Error handling**: `thiserror` in library crates, `anyhow` only in binary crates
- **No unsafe code** — forbidden at workspace level
- **No panics/unwrap/todo/unimplemented** — denied at workspace level
- **No debug output**: `println!`, `eprintln!`, `dbg!` are denied — use `tracing`
- **Secrets**: wrap with `secrecy::Secret<T>`, never log or serialize raw values
- **IDs**: use `uuid::Uuid` (v7 for time-ordered) wrapped in domain newtypes

## Quality Rules

- **No inline lint suppression** — `#[allow()]` and `#[expect()]` are denied.
  To relax a lint for a crate: add to that crate's `[lints.clippy]` section in
  its `Cargo.toml` with a comment explaining why.
- **Max 500 lines per .rs file** — enforced by `just check-lines`
- **Max 100 lines per function** — enforced by clippy `too-many-lines-threshold`
- **Dependencies**: pinned in root `[workspace.dependencies]`, crates reference
  with `dep.workspace = true`. New deps require permissive licenses.

## Dependency DAG (Linking Rules)

These are hard rules — violations should fail code review:

1. **Core rule**: `domain` never imports from any other workspace crate
2. **Transport rule**: interface binaries only depend on `app-services` + `contract`
   (plus runtime/harness crates for composition wiring)
3. **Storage rule**: only `store` owns SQL and query details
4. **Runtime rule**: environment and harness crates never own policy decisions
5. **Policy rule**: policy returns typed decisions, never transport-layer errors
6. **Contract rule**: contract crate is serialization/schema only, no orchestration logic
7. **Observability rule**: no crate emits unstructured logs without correlation context

## Task Runner

Use `just` (not make). Run `just --list` to see all recipes.

```bash
just bootstrap    # first-time setup (installs all tools, idempotent)
just install      # fetch deps + build
just ci           # full local CI (must pass before PR)
just fix          # auto-fix formatting + clippy suggestions
just test         # run tests via nextest
just lint         # clippy with -D warnings
just fmt          # check formatting (Rust + TOML)
```

## Testing

- Use `cargo nextest` (not `cargo test`)
- `insta` for snapshot testing
- `proptest` for property-based testing
- `wiremock` for HTTP mocking
- Unit tests per crate, contract tests for domain/policy invariants
- Integration tests for runtime adapters and database backends

## Build & Development

```bash
just bootstrap    # one-time setup
just install      # fetch + build
just ci           # validate before PR
```

## Commit Style

Conventional Commits with scope:

- `feat(domain): add dispatch lifecycle state machine`
- `feat(store): implement sqlx event append`
- `fix(orchestrator): handle concurrent step guard race`
- `chore: update workspace dependencies`

## Planning Docs

- `docs/rewrite/MOTIVATIONS.md` — why rewrite, pain points, vision
- `docs/rewrite/HLD.md` — high-level architecture, planes, subsystems
- `docs/rewrite/ROADMAP.md` — phased delivery plan with lanes and exit criteria
- `docs/rewrite/DESIGN_PRINCIPLES.md` — 10 decision rules
- `docs/rewrite/CONTAINER_SYSTEM.md` — execution lease lifecycle and security
- `docs/rewrite/RUST_STACK.md` — recommended crate stack
- `docs/rewrite/CRATE_GUIDE.md` — workspace topology, dependency graph, linking rules
