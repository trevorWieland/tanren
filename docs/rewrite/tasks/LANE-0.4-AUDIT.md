# Lane 0.4 — Contract, App-Services, and CLI Skeleton — Audit Brief

## Role

You are auditing the first vertical slice of Tanren 2.0. This lane is
where architectural drift can re-enter the rewrite: interface crates can
accidentally own business logic, orchestrator code can leak transport
concerns, and binaries can bypass the application layer. Your job is to
verify the lane proves the contract-first architecture rather than merely
making the CLI "work somehow."

## Required Reading

Read these before auditing:

1. `docs/rewrite/tasks/LANE-0.4-CLI-WIRING.md`
2. `docs/rewrite/tasks/LANE-0.4-BRIEF.md`
3. `docs/rewrite/DESIGN_PRINCIPLES.md`
4. `docs/rewrite/CRATE_GUIDE.md`
5. `CLAUDE.md`

Skim public exports of:

- `crates/tanren-domain`
- `crates/tanren-store`
- `crates/tanren-contract`
- `crates/tanren-orchestrator`
- `crates/tanren-app-services`

## Audit Dimensions

### 1. Architecture Fidelity

- `tanren-contract` is serialization/schema only.
- `tanren-policy` returns typed decisions, not transport errors.
- `tanren-orchestrator` owns lifecycle sequencing and store coordination.
- `tanren-app-services` maps inputs/outputs and errors; it does not re-implement orchestration.
- `tanren-cli` is thin: argument parsing, dependency wiring, JSON output.

Any inversion here is a blocker because it recreates the drift the rewrite exists to remove.

### 2. Store Boundary Discipline

- Lane 0.4 consumes `tanren-store` only through public traits / param structs / `Store` construction.
- No SQL, entity imports, migration imports, or row-shape assumptions appear outside `tanren-store`.
- No code bypasses the co-transactional store APIs by appending raw events directly.

### 3. Lifecycle Correctness

- `create_dispatch` creates the projection, emits `DispatchCreated`, and enqueues the initial step through the store contract.
- `cancel_dispatch` cancels pending steps, updates status, and emits `DispatchCancelled`.
- Unauthorized `cancel_dispatch` attempts are hidden as `not_found`
  (no cross-scope existence oracle).
- Missing-dispatch and unauthorized cancel attempts both append
  internal `policy_decision` audit events before returning `not_found`.
- Actor-scoped `get` uses scope-predicate-first store reads.
- `cancel_dispatch` authorization is enforced via typed policy checks and
  denied decisions are internally audited before returning masked `not_found`.
- `StateStore::get_dispatch_scoped` is trait-required for every backend
  (no default unscoped fallback implementation).
- Denied create/cancel decisions emit internal `policy_decision` audit events.
- The orchestrator enforces the single-path terminal-event rule:
  - `DispatchCompleted` only for `Outcome::Success`
  - `DispatchFailed` for all other non-cancel terminal outcomes
  - `DispatchCancelled` only for user-initiated cancellation

### 4. Error Mapping

- Domain/store/policy errors are converted into stable contract error responses.
- The CLI prints deterministic JSON on success and failure.
- On failure, stderr contains only a single JSON document (no log prefix/suffix contamination).
- `policy_denied` details are minimized and do not expose resource metadata.
- JWT verification failures returned to clients are generic and do not expose
  issuer/audience/expiry/signature-specific diagnostics.
- Internal `correlation_id` values returned to clients are traceable via
  machine-readable local sink events in CLI mode.
- Internal failures omit `correlation_id` when sink persistence fails.
- Actor token source resolution enforces true one-of across stdin/file/env.
- Scoped-read query/index strategy has backend-native `EXPLAIN` validation
  for both SQLite and Postgres.
- Libraries use `thiserror`; `anyhow` appears only in binaries.

### 5. Test Quality

- Contract serde round-trip tests exist.
- Orchestrator tests cover create/get/list/cancel behavior and terminal-event emission.
- CLI integration tests exercise the full SQLite path: create → get → list → cancel.
- `just ci` is green across the workspace.

## Audit Process

1. Confirm the branch under review is the lane-0.4 branch.
2. Run `just ci`.
3. Inspect crate boundaries and dependency direction.
4. Run the CLI integration tests and confirm they hit the real store path on SQLite.
5. Verify no transport logic leaked into orchestrator and no orchestration logic leaked into the CLI.

## Approve When

- The vertical slice works end to end on SQLite.
- Crate boundaries from `CRATE_GUIDE.md` remain intact.
- Terminal-event emission is single-path and explicitly tested.
- Errors are typed and stable at the contract boundary.
- `just ci` passes with no suppressions or layering regressions.
