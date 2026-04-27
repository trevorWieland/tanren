# Tanren Product Roadmap

## Current Foundation

Tanren has a Rust workspace with strict quality gates, typed domain contracts,
event/store infrastructure, methodology tooling, command installation, MCP
support, and a BDD-owned Phase 0 proof surface.

The codebase is treated as the current Rust product line. New work should
build forward from the Rust crates and roadmap phases.

## Phase 1: Runtime Substrate

Goal: execute dispatches through pluggable harness and environment adapters.

Required capabilities:

- Define a shared harness contract for execution, streaming, tool use, session
  semantics, sandbox rules, and approval mapping.
- Implement initial harness adapters for Claude Code, Codex, and OpenCode with
  normalized telemetry and error classification.
- Define an environment lease contract for local worktrees, local containers,
  Docker-outside-of-Docker, and remote execution targets.
- Implement initial environment adapters for local worktree, local container,
  and Docker-outside-of-Docker flows.
- Add a worker runtime that consumes lanes, applies typed retries, persists
  result artifacts, and recovers from cancellation or crash paths.

Exit criteria:

- One dispatch contract can run across at least two harnesses and two
  environment types.
- Execution signals and errors are normalized across adapters.
- Lease lifecycle covers success, failure, cancellation, and recovery.
- Phase 1 BDD proof scenarios have positive and falsification witnesses.

## Phase 2: Planner-Native Orchestration

Goal: make planning graph execution the native orchestration model.

Required capabilities:

- Add a task/dependency graph model with explicit planner input and output
  contracts.
- Add graph-aware scheduling with lane, capability, and backpressure awareness.
- Add evidence-driven replanning after failure, conflict, or policy denial.
- Add structured artifacts for plans, patches, tests, audits, and outcomes.

Exit criteria:

- Intake can produce a graph and execute it to completion or typed failure.
- Replanning is triggered by defined failure classes.
- Final outputs include machine-readable evidence artifacts.
- Graph revision and event semantics are stable enough for later policy and
  interface work.

## Later Phases

- Policy and governance: scoped identity, budgets, quotas, placement policy,
  and approval gates.
- Interface parity: CLI, API, MCP, and TUI behavior from one contract.
- Scale and operations: read models, observability, multi-tenancy, SLOs, and
  recovery runbooks.
