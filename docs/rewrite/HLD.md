# Tanren Clean-Room Rewrite: High-Level Design

## System Identity

Tanren is an agent orchestration control plane for software delivery.

It decides:

- what work should happen
- in what order
- on which execution substrate
- under which policy constraints

It does not hardcode one harness, one environment, or one interface.

## Architecture Overview

### Two Planes

Tanren is split into two explicit planes:

**Control Plane**

- Planning and decomposition
- Dispatch graph generation and lifecycle governance
- Policy enforcement (budget, quota, permissions, placement)
- Cross-interface contract surface (CLI/API/MCP/TUI)
- Observability and audit

**Execution Plane**

- Harness execution (Claude Code, Codex, OpenCode, future harnesses)
- Environment provisioning and teardown (local, Docker, DooD, remote VM/cloud)
- Artifact streaming and result capture
- Step-level retries and failure recovery

### Three Execution Modes

Tanren orchestrates three categories of work:

1. **Interactive operational actions**
   Manual and tool-driven operations through CLI/API/MCP/TUI.
2. **Automated project workflows**
   Scheduled and event-driven dispatch graphs for issue/task pipelines.
3. **Planning and orchestration loops**
   Planner-guided decomposition, execution, and re-planning based on evidence.

## Core Subsystems

### 1. Domain Model and Contract Layer

The canonical domain model defines:

- entities: project, issue, dispatch, step, environment lease, run artifact, policy decision
- states: lifecycle transitions and terminal conditions
- commands/events: all mutations and system signals
- errors: typed, stable, interface-safe

Every interface is derived from this model:

- CLI command schemas
- API schemas
- MCP tool schemas
- TUI interaction contracts

### 2. Planner and Orchestration Engine

The orchestration engine moves from step-only sequencing to planner-native graphs:

- issue -> work breakdown graph
- dependency-aware scheduling
- lane and capability-aware placement
- dynamic re-planning after failures, conflicts, or policy rejections

Outputs are explicit dispatch graphs with deterministic state transitions.

### 3. Harness Runtime Abstraction

Harnesses are adapters behind a shared contract:

- prompt payload and context contracts
- capability negotiation (streaming, tool use, patch application, session resume)
- telemetry normalization (tokens, duration, retries, errors)
- sandbox/approval behavior mapping

Initial harness set:

- Claude Code
- Codex
- OpenCode

### 4. Environment Runtime Abstraction

Execution environments are adapters behind a shared lease contract:

- local worktree
- local Docker
- DooD from compose
- remote VM providers (Hetzner, GCP, DigitalOcean, future)

Environment leasing includes:

- requested capabilities (CPU/memory/GPU/network profile)
- policy constraints
- cost and quota checks
- lifecycle hooks (setup, checkpoint, teardown)

### 5. Policy and Governance Layer

Policy is evaluated before and during execution:

- identity and authorization
- scope and project boundaries
- budget and quota limits
- allowed harness/environment combinations
- required approvals for high-risk operations

All policy decisions are evented and auditable.

### 6. Store and Eventing Layer

A unified event-sourced store remains the core durability model, but with
explicit read-model strategy for scale:

- append-only canonical events
- write-side transactional guarantees
- indexed read models for status, metrics, VM/environment inventory, and dashboards
- retention and archival policies

### 7. Observability and Audit

First-class telemetry includes:

- dispatch graph progress and bottlenecks
- harness usage and model cost
- environment lease utilization and waste
- policy decision traceability
- user/team/org attribution

## Key Data Flows

### Flow A: Planner-Driven Execution

1. Intake issue/request through CLI/API/MCP/TUI.
2. Planner produces a work graph (tasks, dependencies, required capabilities).
3. Policy engine validates graph against quotas, scopes, and approvals.
4. Scheduler emits dispatches into execution lanes.
5. Runtime selects harness + environment by capability/policy/cost.
6. Worker executes, streams artifacts, emits normalized events.
7. Planner consumes outcomes and either advances, remediates, or replans.
8. Final state and evidence are published to all interfaces consistently.

### Flow B: Manual Operational Dispatch

1. User submits explicit dispatch parameters.
2. Domain contract validation and policy check run synchronously.
3. Dispatch enters queue and executes with selected runtime adapters.
4. Status and artifacts are queryable identically from CLI/API/MCP/TUI.

### Flow C: Environment Lease Lifecycle

1. Request lease with capabilities and policy context.
2. Placement engine resolves substrate and provider.
3. Provisioning executes with traceable cost + policy decision.
4. Lease reused or pinned for multi-step workflow if policy allows.
5. Teardown guaranteed with recovery for stale leases.

## Configuration and Secret Model

### Scope Model

Configuration and secrets are separated and scoped:

- **Project scope**: committed runtime intent and requirements
- **Developer scope**: local preferences and personal credentials
- **Organization scope**: policy, infrastructure, budget, compliance

### Source and Precedence

Sources are layered and explicit with deterministic precedence:

1. Organization policy/config
2. Project config
3. Developer local overrides (where permitted)
4. Runtime environment overrides (bounded)

Secrets are never embedded in project config.

## Deployment Model

Tanren targets three deployment classes with the same architecture:

1. **Solo**: local daemon/store, optional Docker/remote execution
2. **Community project**: shared project config with contributor credentials
3. **Enterprise**: centralized control plane, policy enforcement, budgeting, audit, on-prem/cloud

## Non-Goals

The rewrite does not aim to:

- re-implement every legacy behavior before establishing the new contract foundation
- lock tanren to one harness provider
- force one environment substrate
- preserve undocumented drift between legacy interfaces
