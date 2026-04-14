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
    pub code: String,       // stable error code
    pub message: String,
    pub details: Option<serde_json::Value>,
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
tanren dispatch create  --project <P> --phase <PH> --cli <C> --branch <B> ...
tanren dispatch get     --id <ID>
tanren dispatch list    [--status <S>] [--limit <N>]
tanren dispatch cancel  --id <ID> [--reason <R>]
```

The CLI:
1. Reads `--database-url` flag (default: `sqlite:tanren.db`)
2. Creates a `Store` from the URL
3. Creates the service stack: `Store → Orchestrator → DispatchService`
4. Runs the requested command
5. Prints results as JSON to stdout

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
