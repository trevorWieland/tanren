# Tanren Project Conventions

Tanren is a Rust control plane for agentic software delivery. This file
summarizes repo conventions for Claude-oriented agents; `AGENTS.md` is the
general contributor guide.

## Architecture

The workspace contains Rust libraries in `crates/`, binaries in `bin/`, and
repo automation in `xtask/`.

```text
bin/
  tanrend              # daemon/control-plane runtime
  tanren-api           # HTTP API
  tanren-cli           # CLI
  tanren-mcp           # MCP server
  tanren-tui           # terminal UI

crates/
  tanren-domain        # canonical entities, commands, events, errors
  tanren-contract      # interface schema generation and versioning
  tanren-policy        # authz, budgets, quotas, placement policy
  tanren-store         # event log, projections, migrations
  tanren-planner       # task graph planning and replanning
  tanren-scheduler     # lane/capability-aware dispatch scheduling
  tanren-runtime       # harness and environment trait contracts
  tanren-runtime-*     # concrete environment implementations
  tanren-harness-*     # concrete harness implementations
  tanren-orchestrator  # control-plane orchestration engine
  tanren-observability # tracing, metrics, correlation helpers
  tanren-app-services  # shared service layer for interfaces
```

## Rust Conventions

- Edition: 2024, stable toolchain pinned in `rust-toolchain.toml`.
- Error handling: `thiserror` in libraries, `anyhow` only in binaries and
  repo tooling.
- Unsafe code is forbidden at workspace level.
- `unwrap`, `panic!`, `todo!`, `unimplemented!`, `println!`, `eprintln!`, and
  `dbg!` are denied by lint policy.
- Secrets use `secrecy::Secret<T>` and are never logged or serialized raw.
- IDs use `uuid::Uuid`, usually wrapped in domain newtypes.

## Dependency Rules

1. `tanren-domain` does not depend on other workspace crates.
2. Interface binaries use `tanren-app-services` and `tanren-contract` for
   product behavior.
3. Only `tanren-store` owns SQL and database row details.
4. Runtime and harness crates do not own policy decisions.
5. Policy returns typed decisions, not transport errors.
6. Contract crates are serialization/schema surfaces, not orchestration logic.
7. Observability is structured and correlation-friendly.

## Quality Gate

Use `just`.

```bash
just bootstrap
just install
just fmt
just check
just tests
just ci
```

`just ci` is the PR gate and the required status check for rewrite branches.

## Testing

- `just tests` is the authoritative behavior proof path.
- BDD feature files live under `tests/bdd/features/`.
- The BDD harness is `crates/tanren-bdd`.
- Non-BDD Rust test files and inline `#[cfg(test)]` modules are prohibited
  outside the BDD harness.

## Planning Docs

- `docs/product/vision.md` - product-to-proof vision and boundaries
- `docs/roadmap/dag.json` - roadmap DAG source-of-truth
- `docs/roadmap/roadmap.md` - current human-readable roadmap view
- `docs/behaviors/index.md` - product behavior catalog
- `tests/bdd/README.md` - executable behavior evidence rules
- `docs/architecture/` - durable architecture and interface boundaries
- `docs/architecture/delivery.md` - command installation and delivery system
