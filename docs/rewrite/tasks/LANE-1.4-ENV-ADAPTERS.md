# Lane 1.4 — Initial Environment Adapters

## Status

Planned for Phase 1.

## Purpose

Implement the first environment adapters on top of the Lane 1.3 lease
contract and prove cross-environment semantic equivalence.

Behavioral baseline: [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)
Feature 2, Feature 4, and Feature 6.

## Scope

This lane ships:

- initial environment adapters for local worktree plus local-daemon
  containerized execution
- compatibility signaling per adapter
- typed lease acquisition/cleanup failure mapping
- conformance and parity evidence across shipped environment types
- validation that Phase 1 baseline adapters do not introduce assumptions
  that block future DooD-style container adapters

## Behavioral requirements

1. Equivalent dispatches produce equivalent domain outcomes across
   supported environment types.
2. Incompatible/unavailable environments produce typed denials or failures.
3. Cleanup guarantees hold for success, failure, cancellation, and timeout.
4. Recovery behavior is observable and safe after unexpected interruption.
5. Baseline containerized adapter behavior does not rely on assumptions
   that would invalidate future DooD support.

## Dependencies

- [LANE-1.3-ENV-CONTRACT.md](LANE-1.3-ENV-CONTRACT.md)

## Out of scope

- queue worker retry orchestration and global scheduling policy
- implementing the DooD adapter itself in this lane
