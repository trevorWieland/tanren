# Tanren Clean-Room Rewrite: Roadmap

## Build Philosophy

Each phase must produce a working increment with testable exit criteria.

Phases are sequential for core foundations but can contain independent lanes
developed in parallel. Integration happens at phase boundaries.

## Phase 0 - Foundation

**Goal**: Establish workspace, domain contracts, and event/store skeleton.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 0.1 | Workspace | Rust workspace scaffolding, crate boundaries, CI pipeline (`fmt`, `clippy`, `test`, docs lint), containerized dev toolchain. |
| 0.2 | Domain Contract | Canonical entities/commands/events/errors with versioning strategy and compatibility policy. |
| 0.3 | Store Core | Event log + projection model with transactional append/ack semantics and migration framework. |
| 0.4 | Interface Schema | Contract-derived schema generation pipeline for CLI/API/MCP/TUI bindings. |
| 0.5 | Methodology Boundary | Typed task lifecycle, agent tool surface (MCP + CLI), typed evidence schemas, multi-agent install (Claude Code, Codex Skills, OpenCode), self-hosted `commands/` → rendered artifacts, full orchestration-flow spec. |

### Exit Criteria

- Workspace builds and tests cleanly with strict linting.
- Domain model includes explicit state transition rules and error taxonomy.
- Event append and projection updates are transactional.
- At least one generated interface surface is running from the shared contract.
- Methodology ownership is explicit and typed: workflow mechanics live in
  Rust control-plane code; command markdown remains an agent-behavior layer;
  tasks, findings, rubric scores, and evidence frontmatter are strictly
  typed domain entities.
- `tanren-cli install` renders the shared command source into three agent-
  framework targets (Claude Code, Codex Skills, OpenCode) with
  deterministic, idempotent output; the tanren repo is self-hosting.
- Multi-guard task completion is modeled
  (`gate_checked ∧ audited ∧ adherent` by default), with guards
  configurable and independently executable.

## Phase 1 - Runtime Substrate Core

**Goal**: Run dispatches through pluggable harness and environment adapters.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 1.1 | Harness Adapter Contract | Shared harness trait and capability model (streaming, tools, session semantics, sandbox/approval mapping). |
| 1.2 | Initial Harness Adapters | Claude Code, Codex, OpenCode adapters with normalized telemetry and errors. |
| 1.3 | Environment Adapter Contract | Unified environment lease API for local worktree, local Docker, DooD, remote VM. |
| 1.4 | Initial Environment Adapters | Implementations for local worktree + local Docker + DooD execution. |
| 1.5 | Worker Runtime | Lane consumer runtime with typed retries, recovery, and durable result artifacts. |

### Exit Criteria

- Same dispatch contract can run across at least two harnesses and two environment types.
- Telemetry and errors are normalized across harness adapters.
- Lease lifecycle handles success, failure, cancellation, and crash recovery.

## Phase 2 - Planner-Native Orchestration

**Goal**: Move from step sequencing to graph-based planning and execution.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 2.1 | Planning Graph | Task/dependency graph model and planner I/O contracts. |
| 2.2 | Scheduler | Graph-aware scheduling with lane/capability awareness and backpressure. |
| 2.3 | Replanning Engine | Evidence-driven replan path after failure/conflict/policy denial. |
| 2.4 | Artifact Model | Structured evidence model for plans, patches, tests, audits, and outcomes. |

### Exit Criteria

- Intake request can produce a graph and execute to completion or structured failure.
- Replanning is triggered automatically on defined failure classes.
- Final outputs include machine-readable evidence artifacts.

## Phase 3 - Policy and Governance

**Goal**: Introduce hard governance for scope, budget, and placement.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 3.1 | AuthN/AuthZ | User/key identity model, scoped permissions, interface-level enforcement. |
| 3.2 | Budget and Quotas | Per-user/team/org dispatch, runtime, and cost limits with atomic checks. |
| 3.3 | Placement Policy | Capability and policy-aware environment selection (including GPU constraints). |
| 3.4 | Approval Gates | Configurable manual/automatic approvals for sensitive actions. |

### Exit Criteria

- Policy denials are explicit typed outcomes (no ambiguous failures).
- Budget and quota enforcement is race-safe under concurrency.
- Placement decisions are auditable with rationale.

## Phase 4 - Interface Parity and UX

**Goal**: Guarantee CLI/API/MCP/TUI functional parity from one contract.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 4.1 | CLI | Contract-generated command model with deterministic UX and error mapping. |
| 4.2 | API | Contract-generated/validated HTTP surface with stable versioning. |
| 4.3 | MCP | Tool surface generated from shared contract and capability metadata. |
| 4.4 | TUI | Operator-oriented lifecycle monitor and control actions. |

### Exit Criteria

- Core workflows produce equivalent outcomes across all interfaces.
- Contract drift checks fail CI if interface behavior diverges.
- Error classes map consistently across interfaces.

## Phase 5 - Scale, Observability, and Enterprise Readiness

**Goal**: Make the system reliable at sustained production load.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 5.1 | Read Models | Indexed status/metrics/inventory projections replacing scan-heavy queries. |
| 5.2 | Observability | Traces, structured metrics, and audit event streams with correlation IDs. |
| 5.3 | Multi-Tenancy | Org/team isolation boundaries, attribution, and governance reporting. |
| 5.4 | SLO and Recovery | Defined SLOs, chaos/recovery tests, and operational runbooks. |

### Exit Criteria

- Load tests meet throughput/latency/error objectives.
- No critical scan paths remain for hot operational queries.
- Full audit trail available for policy, execution, and cost events.

## Phase 6 - Migration and Cutover

**Goal**: Replace the existing Python implementation safely with minimal disruption.

### Lanes

| Lane | Area | Deliverable |
|------|------|-------------|
| 6.1 | Compatibility Layer | Existing-to-new command/API compatibility where required for transition. |
| 6.2 | Data Migration | Event/state migration tooling with validation and rollback strategy. |
| 6.3 | Dual-Run Validation | Side-by-side execution comparison for correctness and performance. |
| 6.4 | Cutover | Production rollout playbook, staged enablement, fallback procedure. |

### Exit Criteria

- Migration tools run repeatably with validated parity.
- Dual-run demonstrates acceptable behavior equivalence and improved SLOs.
- Cutover and rollback procedures are tested end-to-end.

## Parallel Workstreams

The rewrite should evolve in parallel with Forgeclaw:

- shared harness contract learnings
- shared environment and policy patterns
- shared observability and audit semantics

Coordination is required, but each project remains independently operable.
