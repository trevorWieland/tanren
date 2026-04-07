# Tanren Clean-Room Rewrite: Motivations

## What Tanren Proved

Tanren proved a Python-first orchestration model can coordinate agent phases,
manage local and remote execution environments, and expose multiple interfaces
(CLI, API, MCP) over a shared queue and event store.

Tanren also proved:

- A dispatch-centric lifecycle is a valid orchestration primitive.
- Lane-based concurrency is useful for separating implementation, audit, and gate work.
- Protocol boundaries (event store, queue, state store, adapters) reduce coupling.
- Project/developer/infrastructure scoping is the right lens for config and secrets.

The concept is real. The current implementation is not the final form.

## What Is Wrong Today

### Interface Parity Is Intent, Not Guarantee

CLI, API, and MCP mostly align conceptually, but behavior and wiring still drift in
real operation. State and lifecycle semantics are not generated from one canonical
domain contract, so divergence reappears over time.

### Config and Runtime Surfaces Are Fragmented

Different processes can target different backing stores and different runtime
assumptions unless manually aligned. This creates operational split-brain risk
and makes debugging difficult across deployment modes.

### Orchestration Is Step-Centric, Not Planner-Centric

Current lifecycle orchestration is strong on provisioning/execution/teardown
mechanics but weak on higher-order agentic planning:

- task graph decomposition
- dependency-aware issue breakdown
- multi-agent strategy selection
- iterative plan refinement based on execution evidence

### Environment Model Is Powerful but Not Policy-First

Tanren supports local, Docker, and remote execution patterns, but resource policy
is not yet a first-class scheduling and governance system:

- placement decisions (local vs container vs VM)
- hard cost ceilings and budget attribution
- quota policies per user/team/org
- required capabilities (GPU, CPU/memory class, network zone) as enforceable constraints

### Secrets and Config Separation Is Correct but Operationally Heavy

The current scoping model is directionally right, but lifecycle and ergonomics are
not yet clean enough for all target audiences:

- solo developers
- open-source contributors with shared repo config + personal credentials
- enterprise teams with centralized policy, auditing, and on-prem controls

### Scale and Operability Are Not Yet Enterprise-Ready

Some core read paths still rely on scanning patterns and in-memory aggregation that
will not hold up under large workload volumes, large team counts, or sustained
automation usage.

## Why a Clean-Room Rewrite

This is not a "port Python to Rust" exercise. It is a redesign around intent.

Incremental refactoring inside the current architecture cannot fully solve:

- cross-interface contract drift
- execution substrate policy modeling
- planner-native orchestration
- long-term multi-tenant governance
- enterprise-grade performance and reliability goals

A clean-room rewrite allows us to:

1. Define one canonical domain contract and generate CLI/API/MCP/TUI bindings from it.
2. Treat harnesses and environments as first-class pluggable runtimes.
3. Separate control-plane policy from data-plane execution.
4. Build planning-first orchestration rather than step-only orchestration.
5. Deliver a system that scales from solo workflows to enterprise operations without forking architecture.

## Why Rust

Rust is selected for correctness, performance, and long-term maintainability.

- Strong typing across domain models and state transitions.
- Reliable async concurrency for high-throughput orchestration.
- Memory safety without runtime GC pauses.
- High confidence infrastructure ecosystem for APIs, queues, storage, and telemetry.
- Explicit modular boundaries via Cargo workspaces and crate-level contracts.

## The Vision

Tanren should become an enterprise-grade agent orchestration platform:

- planner-native, not just executor-native
- harness-agnostic (Claude Code, Codex, OpenCode, future adapters)
- environment-agnostic (local worktree, local Docker, DooD, remote VM/cloud)
- policy-driven (budget, quota, placement, compliance, audit)
- interface-consistent (CLI/API/MCP/TUI from one domain model)
- automation-friendly and deterministic enough for non-interactive workflows

This rewrite is the path from proof-of-concept orchestration to a durable
control plane for real software delivery.
