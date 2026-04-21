# Lane 1.3 — Environment Lease Contract

## Status

Planned for Phase 1.

## Purpose

Define the environment lease contract that normalizes acquisition,
execution context exposure, cleanup, and recovery semantics across
environment types.

Behavioral baseline: [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)
Feature 2 and Feature 4.

## Scope

This lane defines:

- lease lifecycle states and transition semantics
- compatibility/capability checks for environment selection
- typed denial and failure classes for lease operations
- cleanup invariants for success, failure, cancellation, and crash recovery
- conformance expectations every environment adapter must satisfy
- container-topology-agnostic semantics so containerized execution is not
  coupled to one daemon placement model

This lane does **not** ship concrete environment adapters.

## Behavioral requirements

1. Environment choice must not change domain-level execution semantics.
2. Incompatible environment requests are denied safely before execution.
3. Lease cleanup is deterministic across all terminal paths.
4. Crash recovery semantics prevent orphaned or duplicate execution.
5. Contract semantics must remain compatible with future DooD-style
   container adapters without breaking existing adapter contracts.

## Guardrails for Future Container Adapters

When defining contract semantics, do not assume:

- container execution always uses a local Docker daemon
- host and container filesystem identity is always shared
- network/process visibility semantics are identical across adapters

Compatibility and behavior guarantees should be expressed via typed
capabilities and lease metadata, not implicit adapter-specific defaults.

## Dependencies

- [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md)
- [LANE-1.2-HARNESS-ADAPTERS.md](LANE-1.2-HARNESS-ADAPTERS.md)

## Out of scope

- implementing concrete environment adapters
- queue worker orchestration and retry policy
