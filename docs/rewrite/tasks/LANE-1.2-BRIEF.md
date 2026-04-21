# Lane 1.2 — Initial Harness Adapters — Agent Brief

## Task

Ship initial harness adapters using the Lane 1.1 contract and prove
cross-harness semantic equivalence for Phase 1 workloads.

## Read first

1. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) (Feature 1, 3, 6)
2. [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md)
3. [LANE-1.2-HARNESS-ADAPTERS.md](LANE-1.2-HARNESS-ADAPTERS.md)
4. [../DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
5. [../../../CLAUDE.md](../../../CLAUDE.md)

## Deliverables

| Area | Deliverable |
|------|-------------|
| Adapters | Claude Code, Codex, and OpenCode harness adapters implementing Lane 1.1 contract |
| Parity | Cross-harness parity suite for positive and falsification expectations |
| Errors | Stable typed failure mapping for adapter-specific failures |
| Safety | Verified redaction behavior in persisted execution evidence |

## Non-negotiables

1. Adapter parity is measured at domain semantics, not provider wording.
2. Capability denial behavior is preflight-safe and side-effect-free.
3. Redaction guarantees apply consistently across all shipped adapters.
4. All three Phase 1 harnesses are mandatory scope for this lane.
5. Evidence artifacts are reproducible by another engineer.
6. Adapter tests reuse Lane 1.1 conformance helpers from `tanren-runtime`.

## Done when

1. Claude Code, Codex, and OpenCode adapters pass shared contract conformance tests.
2. Cross-harness matrix evidence covers pairwise equivalence across all three.
3. Typed denial/failure behavior is stable across adapter-specific faults.
4. Lane output satisfies its mapped BDD scenarios.
