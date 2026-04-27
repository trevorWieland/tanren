# Phase 2: Planner-Native Orchestration

## Objective

Make task graphs the native planning and execution unit so Tanren can schedule,
observe, and replan work from structured evidence.

## Work Items

- Planning graph: represent tasks, dependencies, graph revisions, capabilities,
  and scheduling constraints.
- Scheduler: execute graph-ready work with lane awareness, capability matching,
  backpressure, and fairness.
- Replanning: detect failure, conflict, blocker, and policy-denial classes and
  produce a revised graph with traceable rationale.
- Artifact model: capture plans, patches, tests, audits, findings, and outcomes
  as structured evidence.

## Acceptance Evidence

- Intake creates a graph with stable IDs and revision semantics.
- Scheduler executes available nodes without violating dependencies.
- Replanning produces an explicit graph revision after supported failure
  classes.
- Evidence artifacts are machine-readable and linked to graph nodes.
