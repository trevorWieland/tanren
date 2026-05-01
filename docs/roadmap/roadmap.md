# Tanren Roadmap

**Generated:** 2026-05-01
**Source of truth:** [`docs/roadmap/dag.json`](dag.json)

## What this is

A dependency-aware DAG of spec-sized work that, when complete, realizes every
accepted behavior in [`docs/behaviors/`](../behaviors) on every interface that
behavior declares. The DAG lets multiple independent streams progress in
parallel while honoring real ordering constraints.

Read [`dag.json`](dag.json) for the canonical structure. This document is a
human-friendly rendering.

## State

| | |
|---|---|
| Milestones | 27 |
| Spec nodes | 232 (1 foundation + 231 behavior) |
| Accepted behaviors | 282 |
| Behaviors covered | 282 (100%) |
| Longest dependency path | 15 nodes |
| Max parallel width | 70 nodes |

Validate with: `python3 scripts/roadmap_check.py`

## Methodology

**Foundation-then-thin-slices.** F-0001 is a one-time scaffolding spec that
brings the repo from "scaffolding only" to "minimum buildable Tanren" with
every subsystem stubbed and every public interface (web, api, mcp, cli, tui)
hosting a hello-world surface. Foundation completes zero behaviors by design.
Every roadmap spec (R-0001 onwards) is a thin behavior slice that fully
completes its declared behaviors on every interface those behaviors
declare — no future spec is gated on "an interface doesn't exist yet".

**Completion definition.** A behavior spec is complete IFF (a) BDD scenarios
with positive and falsification witnesses pass for every behavior in
`completes_behaviors` on every declared interface, AND (b) the subjective
playbook walks end-to-end with human acceptance on every declared interface.

**Cluster, don't enumerate.** Specs bundle 1-4 closely-related behaviors when
they share scaffolding, lifecycle, or proof structure. Specs split when
behaviors capture distinct user-visible outcomes that need independent proof.

## Phases

The DAG isn't strictly phased — work parallelizes — but milestones cluster
into seven thematic phases that approximate a delivery order:

### Phase 1 — Foundation

Bootstrapping the system and the people who use it.

- **M-0001** Account, Identity & Sign-in Foundation (8 behaviors)
- **M-0002** Configuration & Secret Storage (18 behaviors)
- **M-0003** Project Bootstrap & Asset Install (16 behaviors)
- **M-0004** Permissions & Governance (24 behaviors)

### Phase 2 — Planning Method

The plan-product / identify-behaviors / architect-system / craft-roadmap loop
that Tanren uses on itself and on adopting projects.

- **M-0005** Product Planning Method (20 behaviors)
- **M-0006** Implementation Assessment (2 behaviors)
- **M-0007** Spec Shaping & Lifecycle (14 behaviors)
- **M-0008** Spec Readiness & Quality Gates (5 behaviors)

### Phase 3 — Execution Substrate

Provider connections + runtime + the implementation loop itself.

- **M-0009** Provider Integrations — Source Control & CI (7 behaviors)
- **M-0010** Runtime & Worker Contracts (26 behaviors)
- **M-0011** Implementation Loop (11 behaviors)

### Phase 4 — Proof, Quality, Walk

Sequential because each layer needs the previous: proof → quality → walk.

- **M-0012** Behavior Proof Harness (1 behavior — the methodology contract)
- **M-0013** Quality Gates, Audit & Adherence (2 behaviors — most quality-control work is structurally absorbed by M-0008 pre-impl gates, M-0011 loop gates, and M-0014 walk gates; this milestone owns the user-visible findings UI and the codebase-audit trigger)
- **M-0014** Walk, Review & Merge (15 behaviors)

### Phase 5 — Integrations Surface

Outbound and inbound machine contracts.

- **M-0015** External Tracker Integration (8 behaviors)
- **M-0016** Integration Client Contracts (9 behaviors)
- **M-0017** Webhooks & Event Streaming (3 behaviors)

### Phase 6 — Multi-User & Visibility

What teams need on top of the single-user delivery loop.

- **M-0018** Team Coordination (16 behaviors)
- **M-0019** Observation, Dashboards & Reports (29 behaviors)
- **M-0020** Operations & Health (10 behaviors)
- **M-0021** Cross-Interface Continuity & Notifications (4 behaviors)

### Phase 7 — Advanced Method

The closing-the-loop and intelligence layer.

- **M-0022** Repo Understanding & Standards Evolution (8 behaviors)
- **M-0023** Release & Learning Loop (6 behaviors)
- **M-0024** Autonomy Controls (7 behaviors)
- **M-0025** Prioritization & Replanning (4 behaviors)
- **M-0026** Decision Memory, Undo & Recovery (3 behaviors)
- **M-0027** Proactive Analysis & Findings Routing (6 behaviors)

## Critical path

15 nodes — the longest sequential chain through the DAG:

```
F-0001 → R-0001 → R-0019 → R-0073 → R-0076 → R-0081 → R-0120 → R-0123 →
R-0133 → R-0134 → R-0136 → R-0138 → R-0139 → R-0141 → R-0142
```

This is the full deliver loop end-to-end: scaffold → account → project →
spec creation → shape → ready → loop start → walk trigger → walk session →
walk content → accept → PR → CI status → merge → cleanup.

## Parallelism

After F-0001 + R-0001 land, parallelism opens fast:

| Level | Parallel nodes |
|---|---|
| L0 | 1 (foundation) |
| L1 | 2 |
| L2 | 11 |
| L3 | 28 |
| L4 | 38 |
| L5 | 70 (max width) |
| L6 | 36 |
| L7 | 23 |
| L8 | 6 |
| L9-L14 | 1-7 (mostly the walk-and-merge tail) |

L5 supports up to 70 specs in flight simultaneously across many milestones.

## Useful queries

```bash
# Validate the DAG
python3 scripts/roadmap_check.py

# What's ready to start now?
python3 scripts/roadmap_check.py --ready

# All nodes in one milestone
python3 scripts/roadmap_check.py --milestone M-0007

# One node's full info + neighbors
python3 scripts/roadmap_check.py --node R-0120

# Which spec node owns a behavior
python3 scripts/roadmap_check.py --behavior B-0285

# Full coverage map
python3 scripts/roadmap_check.py --coverage-map

# Longest path
python3 scripts/roadmap_check.py --critical-path

# Auto-remove transitively redundant edges
python3 scripts/roadmap_check.py --reduce
```

## Conventions

- **Node IDs** are stable. `F-XXXX` for foundation, `R-XXXX` for behavior.
  Once a node ships, its ID is durable — successors use `supersedes`.
- **`completes_behaviors`** lists behaviors fully proven by this spec.
  Every behavior node has at least one entry. Foundation may have zero.
- **`supports_behaviors`** lists behaviors this spec partially exercises
  but doesn't own completion of.
- **`depends_on`** is acyclic and minimal — transitively redundant edges are
  removed by `--reduce`. Every behavior node has F-0001 as a transitive
  ancestor.
- **`expected_evidence`** lists per-behavior BDD coverage with witnesses
  (`positive` + `falsification`) and the interfaces the proof must cover.
- **`playbook`** is the human-walked acceptance sequence. Subjective; one
  reviewer signs off.
