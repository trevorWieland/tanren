# Lane 1.1 — Harness Adapter Contract — Agent Brief

## Task

Define and land the harness contract layer that guarantees transport- and
provider-agnostic execution semantics for Phase 1.

## Read first

1. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) (Feature 1, 3, 6)
2. [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md)
3. [../DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
4. [../CRATE_GUIDE.md](../CRATE_GUIDE.md)
5. [../../../CLAUDE.md](../../../CLAUDE.md)

## Deliverables

| Area | Deliverable |
|------|-------------|
| Contract | Harness capability model and normalized execution/failure contract |
| Safety | Redaction policy requirements enforced before durable persistence |
| Conformance | Reusable contract test suite proving adapter conformance |
| Scope Guard | Explicit boundary that concrete harness adapters/parity belong to Lane 1.2 |
| Docs | Lane and planning docs updated to reflect final contract guarantees |

## Non-negotiables

1. Harness choice cannot alter domain-level terminal semantics.
2. Unsupported capabilities are denied before side effects.
3. Raw provider failure formats are mapped to typed contract classes.
4. Redaction is applied before output is durably stored.
5. Conformance criteria are executable, not prose-only.
6. Lane verification is gated by `just ci` from repo root (`make`-based checks are legacy and non-authoritative for acceptance).
7. Lane 1.1 acceptance does not require concrete harness adapter crates; adapter implementation and cross-harness parity are Lane 1.2 deliverables.

## Done when

1. Contract is sufficient to evaluate harness compatibility pre-execution.
2. Contract tests encode both positive and falsification expectations.
3. Redaction safety requirements are contractually testable.
4. Lane artifacts are ready for adapter implementation lanes.
5. Lane scope boundary to Lane 1.2 is explicit and testable in docs.
