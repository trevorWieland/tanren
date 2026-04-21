# Lane 1.4 — Initial Environment Adapters — Agent Brief

## Task

Ship initial environment adapters using the Lane 1.3 lease contract and
prove cross-environment semantic equivalence for Phase 1 workloads.

## Read first

1. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) (Feature 2, 4, 6)
2. [LANE-1.3-ENV-CONTRACT.md](LANE-1.3-ENV-CONTRACT.md)
3. [LANE-1.4-ENV-ADAPTERS.md](LANE-1.4-ENV-ADAPTERS.md)
4. [../DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
5. [../../../CLAUDE.md](../../../CLAUDE.md)

## Deliverables

| Area | Deliverable |
|------|-------------|
| Adapters | Local worktree and local-daemon containerized environment adapters |
| Cleanup | Deterministic cleanup behavior on all terminal paths |
| Recovery | Verified interruption/recovery behavior |
| Parity | Cross-environment parity matrix evidence |
| Extensibility | Evidence that baseline adapter behavior keeps the contract compatible with future DooD support |

## Non-negotiables

1. Cross-environment parity is evaluated by domain semantics.
2. Cleanup and recovery are validated as first-class outcomes.
3. Incompatible environment requests fail safely before execution.
4. Baseline containerized adapter behavior must not encode assumptions
   that block future DooD-style adapters.
5. Evidence is reproducible by another engineer.

## Done when

1. At least two environment adapters pass shared conformance tests.
2. Cross-environment equivalence evidence is produced for representative flows.
3. Cleanup/recovery falsification cases are demonstrated and pass.
4. Extensibility checks confirm no baseline assumptions block DooD support.
5. Lane output satisfies its mapped BDD scenarios.
