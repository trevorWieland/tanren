# Tanren Clean-Room Rewrite: Design Principles

These principles are the decision filter for the rewrite. If two designs appear
equivalent, choose the one that aligns better with these principles.

## 1. Contract-First, Interface-Second

Define one canonical domain contract, then generate or strictly validate every
interface against it.

Implication:

- CLI/API/MCP/TUI parity is enforced by construction, not by convention.

## 2. Planner-Native Orchestration

Execution mechanics are necessary but insufficient. Planning, decomposition,
dependency handling, and replanning are first-class orchestration concerns.

Implication:

- "Dispatch a step" is a primitive, not the orchestration strategy.

## 3. Harness-Agnostic by Design

No harness-specific behavior may leak into core orchestration state transitions.

Implication:

- Claude Code, Codex, OpenCode, and future harnesses are adapters with
  capability declarations and normalized telemetry/error mapping.

## 4. Environment-Agnostic by Design

Execution substrate choice is a policy/scheduling decision, not hardcoded flow.

Implication:

- Local worktree, local Docker, DooD, and remote VM/cloud all implement one
  lease lifecycle contract.

## 5. Policy Is a First-Class Runtime

Authorization, budget, quota, and placement are enforced as typed decisions,
not scattered conditional checks.

Implication:

- Every denied action has an explicit decision record and reason.

## 6. Separation of Config and Secrets with Explicit Scopes

Treat configuration and secrets differently, and scope both explicitly across:

- project
- developer
- organization

Implication:

- No secret values in project-committed config.
- Source precedence is deterministic and auditable.

## 7. Deterministic Failure Semantics

Operational preconditions and lifecycle guard failures are domain-level outcomes,
not generic internal errors.

Implication:

- Conflict/precondition/policy failures are explicit and interface-consistent.

## 8. Event-Sourced Durability, Projection-Driven Read Performance

Canonical history is append-only events; operational performance comes from
purpose-built read models.

Implication:

- Avoid scan-heavy operational paths for status, inventory, and metrics queries.

## 9. Isolation, Least Privilege, and Auditability

Assume untrusted execution environments and enforce minimal privilege.

Implication:

- Identity-bound actions
- bounded credentials
- explicit network and resource policies
- auditable policy and runtime trails

## 10. Scale Across Solo, Community, and Enterprise Without Forking Architecture

One architecture must support:

- solo local usage
- community/shared project usage
- enterprise governance and compliance

Implication:

- deployment mode changes should adjust policy and infrastructure, not redesign
  core orchestration semantics.
