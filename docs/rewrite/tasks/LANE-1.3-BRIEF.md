# Lane 1.3 — Environment Lease Contract — Agent Brief

## Task

Define and land the environment lease contract that provides safe,
consistent lifecycle behavior across environment types.

## Read first

1. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) (Feature 2, 4)
2. [LANE-1.3-ENV-CONTRACT.md](LANE-1.3-ENV-CONTRACT.md)
3. [../DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
4. [../CRATE_GUIDE.md](../CRATE_GUIDE.md)
5. [../../../CLAUDE.md](../../../CLAUDE.md)

## Deliverables

| Area | Deliverable |
|------|-------------|
| Contract | Lease lifecycle contract and compatibility model |
| Safety | Deterministic cleanup semantics for all terminal paths |
| Recovery | Crash-recovery behavioral contract (orphan reconciliation, duplicate prevention) |
| Conformance | Executable contract tests for environment adapter compliance |
| Extensibility | Container-topology-agnostic semantics that preserve future DooD adapter support |

## Non-negotiables

1. Cleanup semantics are explicit for success, failure, and cancellation.
2. Incompatible environment selection fails before partial execution.
3. Recovery expectations are testable and deterministic.
4. Contract remains adapter-agnostic and extensible.
5. Contract semantics do not assume a local Docker daemon or shared
   host/container filesystem identity.

## Done when

1. Lease lifecycle and denial/failure classes are unambiguous.
2. Cleanup and recovery invariants are covered by contract tests.
3. Lane output is ready for concrete environment adapter implementation.
4. Future DooD-style adapter compatibility constraints are explicit and testable.
