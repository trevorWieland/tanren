---
schema: tanren.roadmap_view.v0
source: dag.json
status: current
owner_command: craft-roadmap
updated_at: 2026-04-30
---

# Roadmap

This file is the human-readable projection of Tanren's roadmap DAG. The
machine-readable planning source is `dag.json`.

## Current Direction

Tanren is entering a system-architecture revision. The roadmap intentionally
treats the current implementation as evidence to mine, not as a shape to
preserve. Existing code, tests, command markdown, and proof assets may contain
useful primitives, but new executable work should target the accepted
architecture:

- typed event canon in Postgres;
- generated or validated repo projections rather than file-first source of
  truth;
- one shared HTTP control-plane contract for web, API, MCP, CLI, and TUI;
- behavior-backed planning, specs, proof, walks, reviews, and release learning;
- isolated runtime execution through queues, workers, leases, target placement,
  scoped access, and harness adapters;
- governance, configuration, secrets, integrations, observation, and operations
  as core product capabilities.

The implementation readiness assessment shows broad but shallow foundations:
only two behaviors are currently classified as `already_implemented`, eleven
as `close_needs_work`, and most accepted behaviors are `partial_foundation`.
The most important architecture divergence is the interface/control-plane
shape: current API and TUI surfaces are not yet real control-plane clients, and
some CLI/MCP paths are closer to temporary methodology commands than native
typed product state.

## Graph Assumptions

- All nodes are `planned`; the previous DAG contained no completed or in-flight
  nodes to preserve.
- Each executable node completes at least one accepted behavior, and completion
  means the behavior is implemented and asserted.
- A roadmap node cannot be marked complete while any behavior in
  `completes_behaviors` is only implemented, unproven, or missing demo/walk
  evidence.
- Dependencies are intentionally front-loaded around state, contracts, scope,
  and planning because later orchestration and runtime behavior should not be
  built on file-first shortcuts.
- Existing implementation should be salvaged only when it aligns with the
  accepted event, contract, projection, policy, and runtime boundaries.

## Completion Definition

In this roadmap, **a spec completes a behavior** means the spec completes and
asserts that behavior. Implementation alone is insufficient.

Before a roadmap node can move to `complete`, every behavior in
`completes_behaviors` must have:

- executable behavior proof linked to that behavior;
- a positive witness;
- a meaningful falsification witness unless the shaped spec explicitly
  justifies why none applies;
- demo or walk evidence showing the behavior is real through the observable
  surface named by the behavior;
- assertion status visible through native behavior-proof and assessment read
  models once those subsystems exist.

Before the native behavior-proof subsystem exists, early nodes may satisfy the
assertion requirement through repo BDD proof and the shaped spec walkthrough
record. After native behavior proof exists, completion also requires the
assertion to be visible through Tanren's own proof and assessment state.

## Parallelization Strategy

The graph keeps only three nodes in the serial kernel: event/read-model state
(`R-0001`), shared HTTP contracts (`R-0002`), and bootstrap scope (`R-0003`).
After that, work splits into planning, governance/configuration, runtime,
provider integration, interface, delivery-asset, and observation tracks.

The fast path to meaningful Tanren-in-Tanren is:

```text
R-0001 -> R-0002 -> R-0003
-> R-0005 -> R-0006 -> R-0007 -> R-0008 -> R-0009
-> R-0011 -> R-0012 -> R-0013
```

In parallel with that planning spine, governance and runtime should move
toward:

```text
R-0026 -> R-0028 -> R-0029 -> R-0030
-> R-0017 -> R-0018 -> R-0019
```

Those two tracks converge at `R-0014`, where Tanren can start asserting real
implementation-loop behavior against shaped specs. From there, `R-0015`,
`R-0021`, `R-0022`, and `R-0023` make iterative spec work easier by adding
blockers, behavior proof, quality controls, and walk/demo records.

## Milestones

### M-0001 Rebuild the control-plane kernel

Goal: replace file-first and direct-service paths with typed event canon,
shared contracts, and coherent public surfaces.

| Node | Title | Depends on |
|---|---|---|
| `R-0001` | Build canonical event log and read-model substrate | none |
| `R-0002` | Stand up shared HTTP interface contracts | `R-0001` |
| `R-0003` | Bootstrap identity, account, organization, and project scope | `R-0001`, `R-0002` |
| `R-0004` | Provide first-party surface shells over the shared contract | `R-0002`, `R-0003` |

This milestone is the architectural reset point. It should not attempt to
complete every interface workflow. It should make the correct path unavoidable:
commands append typed events, read models expose freshness, public surfaces use
the same contracts, and unsupported actions fail explicitly.

### M-0002 Make planning native

Goal: turn product, behavior, architecture, assessment, roadmap, and decision
memory into first-party typed planning state.

| Node | Title | Depends on |
|---|---|---|
| `R-0005` | Implement native product brief and decision memory | `R-0003` |
| `R-0006` | Implement native behavior catalog and coverage state | `R-0005` |
| `R-0007` | Implement architecture records and tradeoff review | `R-0005`, `R-0006` |
| `R-0008` | Implement repository understanding and implementation assessment | `R-0006`, `R-0007` |
| `R-0009` | Implement roadmap DAG and replanning | `R-0006`, `R-0007`, `R-0008` |
| `R-0010` | Implement planning proposal, revision recovery, and controlled undo | `R-0005`, `R-0006`, `R-0007`, `R-0009` |

This milestone retires the temporary planning-command posture. Markdown and
JSON remain important repository projections, but accepted product direction
comes from typed planning events and reviewable revisions.

### M-0003 Shape executable specs

Goal: convert accepted roadmap nodes and intake signals into spec-sized work
with readiness, quality, and dependency rules.

| Node | Title | Depends on |
|---|---|---|
| `R-0011` | Implement draft spec intake and candidate creation | `R-0009` |
| `R-0012` | Implement shape-spec readiness and spec quality gates | `R-0011` |
| `R-0013` | Implement spec lifecycle, grouping, and dependency state | `R-0012` |

The current code has useful shaping primitives, especially around acceptance
criteria and prioritization. This milestone should lift the good parts into a
native spec model that is anchored to accepted roadmap nodes and proof
obligations.

### M-0004 Run the orchestration loop

Goal: execute active specs through loop state, blockers, notifications, team
coordination, and candidate implementations.

| Node | Title | Depends on |
|---|---|---|
| `R-0014` | Implement loop start, state, pause, resume, cancellation, and eligibility | `R-0013`, `R-0019` |
| `R-0015` | Implement blockers, notifications, and attention routing | `R-0014` |
| `R-0016` | Implement live activity and team coordination | `R-0014`, `R-0015`, `R-0026` |

The readiness assessment identifies several loop behaviors as close but
architecture-divergent. They should be rewritten through shared orchestration
state rather than patched in place.

### M-0005 Isolate runtime execution

Goal: run assignments through scoped workers, durable queues, harness
adapters, target placement, and redacted results.

| Node | Title | Depends on |
|---|---|---|
| `R-0017` | Implement runtime harness configuration and readiness | `R-0028`, `R-0029` |
| `R-0018` | Implement isolated execution target placement | `R-0017`, `R-0030` |
| `R-0019` | Implement assignment queue, lease, retry, cancellation, and recovery | `R-0018` |
| `R-0020` | Implement worker-scoped access and redacted runtime output | `R-0019`, `R-0029` |

This milestone implements the accepted runtime posture: no unmanaged local
worktree execution as the core path, workers communicate through public
contracts, provider credentials stay in the control plane, and outputs are
bounded and redacted before persistence.

### M-0006 Prove and review delivered behavior

Goal: require behavior proof, quality controls, walks, PR validation, merge
handoff, and release learning.

| Node | Title | Depends on |
|---|---|---|
| `R-0021` | Implement executable behavior proof | `R-0006`, `R-0012`, `R-0019` |
| `R-0022` | Implement quality controls, findings, audits, and proactive analysis | `R-0008`, `R-0021` |
| `R-0023` | Implement walk, demo, and acceptance records | `R-0014`, `R-0021`, `R-0022` |
| `R-0024` | Implement PR, CI, review feedback, merge-ready, and cleanup flow | `R-0023` |
| `R-0025` | Implement release learning and shipped outcomes | `R-0024` |

This milestone closes the product-to-proof loop. Completion should mean a
behavior is implemented, proven, walked where required, reviewed, linked to a
candidate change, and able to feed release outcomes back into planning.

### M-0007 Govern access and configuration

Goal: add the identity, policy, approvals, configuration, secret, and
credential model needed for safe solo and team use.

| Node | Title | Depends on |
|---|---|---|
| `R-0026` | Implement organization access, roles, memberships, and project grants | `R-0003` |
| `R-0027` | Implement approvals and autonomy boundaries | `R-0026` |
| `R-0028` | Implement configuration inheritance and standards policy | `R-0026` |
| `R-0029` | Implement secrets, credentials, service accounts, and API keys | `R-0026`, `R-0028` |
| `R-0030` | Implement runtime placement, harness, and budget policy | `R-0027`, `R-0028`, `R-0029` |

Governance is not an enterprise add-on. The same model supports solo use,
team use, service accounts, worker-scoped access, approvals, and policy
explanations.

### M-0008 Integrate providers and clients

Goal: connect provider integrations, external trackers, webhooks, API clients,
and source-control status through adapter-backed contracts.

| Node | Title | Depends on |
|---|---|---|
| `R-0031` | Implement provider integration management | `R-0029` |
| `R-0032` | Implement external tracker and outbound issue integration | `R-0011`, `R-0031` |
| `R-0033` | Implement webhooks, external client attribution, CI status, and provider backpressure | `R-0002`, `R-0031` |

Provider-specific payloads and mechanics should stay behind adapters. The
control plane stores normalized state, attribution, health, permissions,
external references, and audited actions.

### M-0009 Expose observation and operations

Goal: provide provenance-aware progress, quality, risk, health, reporting,
backup, restore, incident, and audit views.

| Node | Title | Depends on |
|---|---|---|
| `R-0034` | Implement project, roadmap, blocked-work, and provenance observation | `R-0004`, `R-0009`, `R-0015`, `R-0022`, `R-0037` |
| `R-0039` | Implement pipeline, quality, health, and delivery observation | `R-0016`, `R-0019`, `R-0022`, `R-0024`, `R-0025` |
| `R-0035` | Implement shipped-outcome, report, digest, and changed-since observation | `R-0025`, `R-0034` |
| `R-0036` | Implement operations, backup, restore, pause, incident, quota, and audit export | `R-0027`, `R-0034`, `R-0035` |

Observation claims should show source, freshness, completeness, bounds,
redaction, and whether a value is measured, estimated, inferred, unavailable,
or hidden. Missing data is not healthy data. Planning and blocked-work
observation intentionally lands before delivery metrics so Tanren-in-Tanren can
get useful roadmap and blocker visibility earlier.

### M-0010 Package Tanren for self-hosting

Goal: deliver the Compose-first stack, repository assets, standards profiles,
installer drift handling, upgrade, and uninstall flows.

| Node | Title | Depends on |
|---|---|---|
| `R-0037` | Implement repository asset bootstrap, drift, generated integrations, and standards root | `R-0007`, `R-0022`, `R-0028` |
| `R-0038` | Implement self-hosted stack packaging, upgrade, and uninstall | `R-0003`, `R-0037` |

Delivery should preserve the target architecture instead of introducing a
local-only backend. Compose is the baseline bundle, not a lock-in; the same
service contracts should work under equivalent container orchestrators.

## Evidence Expectations

Each node in `dag.json` declares expected evidence. Before any node is marked
complete, the shaped spec must add or update behavior-level proof for each
completed behavior. The expected default is:

- one behavior-linked BDD target per accepted behavior where practical;
- a positive witness;
- a meaningful falsification witness unless explicitly not applicable;
- proof results and assertion status visible through behavior-proof and
  assessment read models;
- source links from roadmap nodes, specs, proof, walks, reviews, and shipped
  outcomes.

## Preserved Work

No completed or in-flight roadmap nodes existed in the previous DAG. Existing
asserted behaviors remain important source signals:

- `B-0068` Bootstrap Tanren assets into an existing repository;
- `B-0069` Detect installer drift without mutating files;
- `B-0070` Generate selected agent integrations deterministically;
- `B-0071` Use the repository's installed standards;
- `B-0080` See unresolved check findings that block readiness.

Those behaviors are placed in later graph nodes because the architecture
revision should preserve their useful proof while re-homing the implementation
under the event, projection, configuration, and quality-control model.

## Evidence Gaps And Open Decisions

- The roadmap is comprehensive but still graph-level. Each node needs a later
  `shape-spec` pass before implementation.
- The current readiness assessment was static only; it did not run `just
  tests`, `just check`, or `just ci`.
- The graph assumes the accepted architecture remains the target. If
  architecture changes during the mass revision, `R-0007`, `R-0009`, and
  dependent nodes should be replanned through the alteration funnel.
- The exact service split may evolve, but first-party public clients should
  continue to use the HTTP control plane.
