# Lane 0.4 — Contract, App-Services, and CLI Skeleton

## Goal

Prove the contract-first pipeline end-to-end: domain types flow through a
service layer into a CLI that can create a dispatch, store it, and read it back.
This is the first working vertical slice of Tanren 2.0.

**Depends on:** Lane 0.2 (tanren-domain) — needs all domain types.

**Can run in parallel with:** Lane 0.3 (store). The CLI wiring will need the
store at integration time, but the contract/app-services/CLI structure can be
built against the store traits (not the implementation).

## Crates Touched

1. `crates/tanren-contract` — interface schema types
2. `crates/tanren-policy` — minimal policy skeleton (pass-through for now)
3. `crates/tanren-orchestrator` — dispatch creation orchestration
4. `crates/tanren-app-services` — use-case service layer
5. `crates/tanren-observability` — tracing setup
6. `bin/tanren-cli` — clap-based CLI

## Design Constraints

- **Contract-first**: CLI command shapes are derived from domain/contract types,
  not the other way around
- **`thiserror` in libraries, `anyhow` in the CLI binary**
- **No runtime/harness logic yet** — this lane only wires dispatch creation,
  listing, and status queries (no execution)
- **Trait-based store dependency** — the CLI takes a `Store` via dependency
  injection (constructor), not a hardcoded implementation
- **No methodology templating or self-hosting mechanics** — command rendering,
  workflow-context artifacts, issue-source-backed workflow prep, and manual
  Tanren-in-Tanren flow are lane 0.5 concerns

## Hardening Baseline (Post-Audit)

These are mandatory lane-0.4 behaviors for parity/security/stability:

1. **Canonical input semantics live in contract conversion**
   - Request-shape validation (`project_env` key format, `required_secrets`
     name format, duplicate secret rejection) is enforced in
     `tanren-contract` conversion logic.
   - Empty `project_env` values (`KEY=`) are valid.
   - Interface binaries (CLI/API/MCP/TUI) must not implement divergent
     business semantics; they only parse transport shape.

2. **Cancel authorization is explicit and typed**
   - `tanren-policy` exposes a typed cancel decision path.
   - Orchestrator enforces cancel policy before store mutation.
   - Unauthorized cancel attempts are hidden as `not_found` to avoid
     cross-scope existence disclosure.
   - Scope-match model: org must match; if dispatch actor has
     `project_id`/`team_id`/`api_key_id`, canceller must match each present scope.

3. **Cancel transition truth lives in the store transaction**
   - Orchestrator does not pre-check cancel transitions.
   - Store CAS and transition checks are the source of truth.
   - Contention/lock DB errors in cancel path are normalized to stable conflict
     semantics at the store boundary (no backend-specific lock errors leaking up).
   - Normalization uses backend-typed DB codes, not substring matching:
     SQLite `BUSY/LOCKED` code families, Postgres SQLSTATE `40P01/40001/55P03`.

5. **Typed conflict wire codes are deterministic**
   - `invalid_transition` and `contention_conflict` are distinct machine codes.
   - Generic `conflict` is reserved for uncategorized legacy conflict paths.

4. **Step response is enum-typed**
   - Contract `StepResponse` uses enums for step type/status/ready-state/lane.
   - Wire shape remains snake_case for backward-compatible JSON contracts.

6. **Trusted actor context is token-derived (breaking)**
   - `CreateDispatchRequest` and `CancelDispatchRequest` no longer carry
     actor identity fields.
   - CLI requires a signed actor JWT and Ed25519 public-key verification
     inputs (`--actor-token-stdin` or `--actor-token-file` or
     `TANREN_ACTOR_TOKEN`, plus `--actor-public-key-file`,
     `--token-issuer`, `--token-audience`).
   - Missing/invalid tokens fail closed; there is no insecure fallback.
   - Token source resolution is strict one-of across all three sources:
     `--actor-token-stdin`, `--actor-token-file`, `TANREN_ACTOR_TOKEN`.
     Any multi-source combination is rejected.
   - Verification failure responses are generic at the wire boundary
     (`token validation failed`); detailed JWT failure causes stay internal.
   - `get`/`list` are policy-scoped by trusted actor context, not open reads.
   - Actor-token signature/claim verification runs before any store open,
     migration, or schema-readiness work on both read and write command paths.
   - Replay semantics are command-policy aware:
     - `dispatch get/list`: verify-only (no replay consumption write)
     - `dispatch create/cancel`: single-use replay consumption
       atomically within the mutation transaction.

7. **Migration behavior is explicit**
   - Read commands (`dispatch get/list`) open DB without running migrations.
   - Write commands (`dispatch create/cancel`) run migrate-before-write,
     but only after actor-token verification succeeds.
   - `tanren db migrate` is the explicit schema mutation command.
   - Read commands against non-ready schema return `schema_not_ready`.

8. **CLI failure output is deterministic JSON-only**
   - On non-zero exit, stderr is a single JSON document and contains no
     tracing/log prefix/suffix bytes.
   - Internal/store failures preserve `code = internal`, generic
     `message = "internal error"`.
   - `details.correlation_id` is returned only when correlated sink
     persistence succeeds; if sink persistence fails, `correlation_id`
     is omitted.
   - Correlated internal error events are emitted to default JSONL sink:
     `$XDG_STATE_HOME/tanren/internal-errors.jsonl` (fallback:
     `$HOME/.local/state/tanren/internal-errors.jsonl`).

9. **Policy-denied wire details are minimized**
   - `policy_denied` details expose only machine-safe
     `reason_code` (when available).
   - Resource identifiers and internal decision metadata are not exposed.

10. **Scoped dispatch list uses one predicate query**
   - Policy-scoped dispatch reads execute as a single query with
     tuple-aware null-or-exact scope predicates.
   - Single-dispatch reads used by actor-scoped `get` are
     scope-predicate-first.
   - `cancel` authorization is enforced by typed policy checks against
     dispatch ownership scope; denied decisions are audited and still
     returned as masked `not_found`.
   - `StateStore::get_dispatch_scoped` is a required backend contract
     (no default fallback to unscoped read + in-memory filtering).
   - Cursor filtering and ordering stay in SQL
     (`created_at DESC, dispatch_id DESC`) without in-memory fan-out merge/dedupe.
   - Index strategy is validated with backend-native `EXPLAIN` coverage
     for both `SQLite` and `Postgres`.

11. **Denied policy decisions are internally auditable**
   - Denied create/cancel decisions append `DomainEvent::PolicyDecision`
     records to the event log.
   - Both unauthorized cancel attempts and missing-dispatch cancel attempts
     are internally audited, while the wire response remains masked
     `not_found`.
   - Wire responses remain minimized (`policy_denied` details and masked
     `not_found` for unauthorized cancel).

## Deliverables

### 1. Contract Types (`crates/tanren-contract`)

Interface-safe request/response types that map to/from domain types:

```rust
// Request types (what the CLI/API sends)
pub struct CreateDispatchRequest {
    pub project: String,
    pub phase: Phase,
    pub cli: Cli,
    pub branch: String,
    pub spec_folder: String,
    pub workflow_id: String,
    pub mode: DispatchMode,
    pub timeout_secs: u64,
    pub environment_profile: String,
    // ... optional fields
}

// Response types (what the CLI/API receives)
pub struct DispatchResponse {
    pub dispatch_id: String,  // String, not DispatchId — interface-safe
    pub status: String,
    pub mode: String,
    pub lane: String,
    pub created_at: String,
    // ...
}

pub struct DispatchListResponse {
    pub dispatches: Vec<DispatchResponse>,
}

pub struct StepResponse { ... }

pub struct ErrorResponse {
    pub code: ErrorCode,    // serde snake_case (invalid_transition, contention_conflict, ...)
    pub message: String,
    pub details: Option<ErrorDetails>,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    PolicyDenied { reason_code: PolicyReasonCode },
    Internal { correlation_id: Uuid },
}
```

Add `From` impls to convert between domain types and contract types.

### 2. Policy Skeleton (`crates/tanren-policy`)

Minimal for now — a pass-through policy that approves everything:

```rust
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn check_dispatch_allowed(&self, _request: &CreateDispatchRequest) -> Result<PolicyDecision, DomainError> {
        Ok(PolicyDecision::allowed("no policy configured"))
    }
}

pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}
```

This will be expanded in Phase 3. For now it unblocks the orchestrator wiring.

### 3. Orchestrator Skeleton (`crates/tanren-orchestrator`)

Wire the dispatch creation flow using store traits:

```rust
pub struct Orchestrator<S: EventStore + JobQueue + StateStore> {
    store: S,
    policy: PolicyEngine,
}

impl<S: EventStore + JobQueue + StateStore> Orchestrator<S> {
    pub async fn create_dispatch(&self, cmd: CreateDispatch) -> Result<DispatchResult, DomainError> {
        // 1. Policy check
        // 2. Build dispatch from command
        // 3. Create dispatch projection
        // 4. Append DispatchCreated event
        // 5. Enqueue provision step
        // 6. Return dispatch_id + step_id
    }

    pub async fn get_dispatch(&self, id: &DispatchId) -> Result<Option<DispatchView>, DomainError> {
        // Delegate to state store
    }

    pub async fn list_dispatches(&self, filter: DispatchFilter) -> Result<Vec<DispatchView>, DomainError> {
        // Delegate to state store
    }

    pub async fn cancel_dispatch(&self, id: &DispatchId, reason: Option<String>) -> Result<(), DomainError> {
        // 1. Cancel pending steps
        // 2. Update dispatch status to Cancelled
        // 3. Append DispatchCancelled event
    }
}
```

Use domain guard functions (`check_execute_guards`, etc.) from `tanren-domain`.

#### Terminal-event emission rule (carried from Lane 0.2 audit)

The domain schema allows two ways to reach a "dispatch finished
unsuccessfully" state: `DispatchFailed{outcome: Outcome::Error}` or
`DispatchCompleted{outcome: Outcome::Fail}`. The Python system had this
exact duplication and it created projection bugs where different
consumers counted failures differently.

**Orchestrator rule:**

- `DispatchCompleted` is emitted **only** for `Outcome::Success`.
- All non-success terminations (`Fail`, `Blocked`, `Error`, `Timeout`)
  go through `DispatchFailed`.
- `DispatchCancelled` covers user-initiated cancellation and is not
  mixed with `DispatchFailed`.

This rule must be enforced in the orchestrator — the domain model
permits both paths by design (so projections can reconstruct legacy
state) but production emission is single-path.

### 4. App-Services Layer (`crates/tanren-app-services`)

Thin adapter between orchestrator (domain types) and interfaces (contract types):

```rust
pub struct DispatchService<S: EventStore + JobQueue + StateStore> {
    orchestrator: Orchestrator<S>,
}

impl<S: EventStore + JobQueue + StateStore> DispatchService<S> {
    pub async fn create(&self, req: CreateDispatchRequest) -> Result<DispatchResponse, ErrorResponse> {
        // 1. Convert contract request → domain command
        // 2. Call orchestrator.create_dispatch()
        // 3. Convert domain result → contract response
        // 4. Map domain errors → error responses
    }

    pub async fn get(&self, dispatch_id: &str) -> Result<Option<DispatchResponse>, ErrorResponse> { ... }

    pub async fn list(&self, filter: DispatchListFilter) -> Result<DispatchListResponse, ErrorResponse> { ... }

    pub async fn cancel(&self, dispatch_id: &str, reason: Option<String>) -> Result<(), ErrorResponse> { ... }
}
```

### 5. Observability Bootstrap (`crates/tanren-observability`)

Minimal tracing setup that the CLI binary uses:

```rust
pub fn init_tracing(level: &str) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .with_target(false)
        .init();
    Ok(())
}
```

Enough to get structured logs in the CLI. Full OpenTelemetry integration is Phase 5.

### 6. CLI Binary (`bin/tanren-cli`)

Clap-based CLI with these subcommands:

```
tanren dispatch create  --project <P> --phase <PH> --cli <C> --branch <B> ... \
  --actor-token-file <PATH> --actor-public-key-file <PEM> \
  --token-issuer <ISS> --token-audience <AUD>
tanren dispatch get     --id <ID> --actor-token-file <PATH> ...
tanren dispatch list    [--status <S>] [--limit <N>] --actor-token-file <PATH> ...
tanren dispatch cancel  --id <ID> [--reason <R>] --actor-token-file <PATH> ...
tanren db migrate
```

The CLI:
1. Reads `--database-url` flag (default: `sqlite:tanren.db`)
2. For `dispatch` commands, verifies a signed actor JWT into trusted request context
3. Opens store based on command mutability:
   - read (`get/list`): open-only + schema readiness check
   - write (`create/cancel`): migrate-before-write
4. Creates the service stack: `Store → Orchestrator → DispatchService`
5. Runs the requested command
6. Prints results as JSON to stdout

This proves the full pipeline: CLI args → contract types → domain commands →
orchestrator → store → query → contract response → CLI output.

## Testing

- **Contract type round-trip**: serialize → deserialize for all request/response types
- **Orchestrator unit tests**: mock store traits, verify dispatch creation flow
- **CLI integration test**: run CLI binary against in-memory SQLite, verify
  `create` → `get` → `list` → `cancel` flow works end-to-end
- **Error mapping**: verify domain errors map to correct error response codes

## Exit Criteria

- `tanren dispatch create` succeeds against a fresh SQLite database
- `tanren dispatch get` returns the created dispatch
- `tanren dispatch list` shows the dispatch
- `tanren dispatch cancel` transitions to Cancelled
- Full round-trip works: create → get → list → cancel
- `cargo test -p tanren-contract -p tanren-orchestrator -p tanren-app-services -p tanren-cli` passes
- `just ci` passes across the full workspace
- No `unwrap()`, `todo!()`, `panic!()` in library code (`anyhow` is fine in `tanren-cli`)

## Reference (Do NOT Port)

- `services/tanren-cli/src/tanren_cli/main.py` — Python CLI structure
- `services/tanren-api/src/tanren_api/services/dispatch.py` — dispatch service
- `packages/tanren-core/src/tanren_core/dispatch_orchestrator.py` — orchestration logic

Key differences from Python:
- CLI uses clap derive, not typer
- Contract types are explicit structs, not Pydantic models
- Service layer is generic over store traits, not injected at runtime
- Error mapping is typed, not exception-based
