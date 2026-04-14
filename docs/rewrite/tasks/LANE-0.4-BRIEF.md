# Lane 0.4 — Contract, App-Services, and CLI Skeleton — Agent Brief

## Task

Implement the first end-to-end vertical slice of Tanren 2.0 across the
contract, policy, orchestrator, app-services, observability, and CLI
crates. The goal is to prove the contract-first pipeline: create a
dispatch, persist it through the store traits, and read it back through
the CLI.

## Full Spec

Read `docs/rewrite/tasks/LANE-0.4-CLI-WIRING.md` completely before
starting. Also read:

1. `docs/rewrite/DESIGN_PRINCIPLES.md`
2. `docs/rewrite/CRATE_GUIDE.md`
3. `CLAUDE.md`

Lane 0.3 is now the real persistence boundary. Treat its public traits
and param types as the store contract; do not bypass them with SQL or
new persistence helpers.

## Key Context

- **Clean-room rewrite, not a port.** Python CLI/API/orchestration code is
  conceptual reference only.
- **Domain is frozen.** `tanren-domain` is merged and stable.
- **Store is real.** `tanren-store` owns all SQL and persistence semantics.
  Lane 0.4 must call it through traits and param structs, not by reaching
  into entities or migrations.
- **Contract-first means interface-safe types.** `tanren-contract`
  defines request/response types; `tanren-app-services` maps to/from
  domain types; binaries stay transport-specific.
- **No runtime execution yet.** This lane wires dispatch creation,
  query, list, and cancel only. No harness or environment execution
  belongs here.
- **No methodology-boundary work here.** Command templating, self-hosting
  workflow mechanics, issue-source integration, and installed-command
  rendering belong to lane 0.5.

## Deliverables

| Area | Deliverable |
|------|-------------|
| `tanren-contract` | Interface-safe request/response/error types with serde round-trip tests |
| `tanren-policy` | Minimal typed pass-through policy skeleton |
| `tanren-orchestrator` | Dispatch creation/query/list/cancel flow over `EventStore + JobQueue + StateStore` |
| `tanren-app-services` | Service layer mapping contract types to domain/use-case calls |
| `tanren-observability` | Minimal tracing bootstrap used by binaries |
| `bin/tanren-cli` | Clap CLI implementing `dispatch create/get/list/cancel` |

## Non-Negotiables

1. **Thin binaries.** `tanren-cli` parses args, builds dependencies, and prints JSON. Business logic stays in app-services/orchestrator.
2. **No store leakage.** Lane 0.4 must not import `tanren_store::entity`, migrations, or raw SQL.
3. **Single-path unsuccessful termination.** The orchestrator emits:
   - `DispatchCompleted` only for `Outcome::Success`
   - `DispatchFailed` for `Fail | Blocked | Error | Timeout`
   - `DispatchCancelled` only for user-initiated cancellation
4. **Typed error mapping.** Libraries use `thiserror`; the CLI binary may use `anyhow`.
5. **Trait-based wiring.** Orchestrator/service code is generic over store traits or accepts trait objects; do not hardcode SQLite logic into use cases.

## Implementation Order

1. Define contract request/response/error shapes.
2. Implement the minimal policy skeleton.
3. Implement orchestrator create/get/list/cancel against the store traits.
4. Implement app-services mapping and error translation.
5. Add observability bootstrap.
6. Wire the CLI binary to the real `Store`.
7. Add end-to-end CLI tests against SQLite.

## Done When

1. `tanren dispatch create` succeeds against a fresh SQLite database.
2. `tanren dispatch get` returns the created dispatch.
3. `tanren dispatch list` shows the dispatch.
4. `tanren dispatch cancel` transitions to `Cancelled`.
5. Orchestrator tests verify the terminal-event emission rule.
6. Contract types round-trip via serde.
7. CLI integration tests cover create → get → list → cancel.
8. `just ci` passes across the full workspace.

## Out of Scope

- Runtime / harness execution
- Planner-native graph scheduling
- Auth, quotas, and placement policy beyond the minimal skeleton
- API, MCP, or TUI transport wiring
