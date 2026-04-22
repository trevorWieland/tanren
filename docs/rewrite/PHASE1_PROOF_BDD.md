# Phase 1 Proof (BDD)

## Purpose

This document defines the expected **Phase 1 behavior** in usage terms.
It intentionally avoids implementation details.

Audience: technical teammates validating runtime-substrate outcomes from an
operator point of view.

---

## Phase 1 Story

By the end of Phase 1, Tanren should behave like a reliable
**execution substrate** for the control plane shipped in Phase 0:

- the same dispatch contract can execute across multiple harnesses
- the same dispatch contract can execute across multiple environment types
- execution signals and errors are normalized regardless of adapter internals
- lease lifecycle is safe under success, failure, cancellation, and crash recovery
- worker execution is durable and policy-driven (including retries)

Phase 1 is **not** claiming planner-native graph orchestration,
advanced governance, or full interface parity. Those remain later-phase scope.

---

## Proof Model

Each Phase 1 promise is accepted only when both witnesses exist:

1. **Positive witness**: expected behavior works in a realistic flow.
2. **Falsification witness**: invalid or unsafe behavior is rejected safely.

---

## Explicit Phase 1 Invariants

These are hard acceptance invariants (not interpretation notes). Each one must
be backed by both positive and falsification witnesses.

### Invariant I1: Shared authoritative DB truth across isolated runtimes

- Definition:
  - CLI, MCP, harness workers, and replay readers observe one authoritative
    event/store truth for the same dispatch/spec scope.
  - Terminal state and lifecycle evidence queried from different runtime
    surfaces must converge to the same domain truth.
- Positive witnesses:
  - Feature 1 Scenario 1.1
  - Feature 1 Scenario 1.2
  - Feature 6 Scenario 6.1
- Falsification witnesses:
  - Feature 1 Scenario 1.3
  - Feature 2 Scenario 2.3
  - Feature 4 Scenario 4.3

### Invariant I2: Typed event parity across CLI/MCP/runtime boundaries

- Definition:
  - Equivalent operations emitted through different runtime boundaries map to
    equivalent typed lifecycle semantics and queryable evidence classes.
  - Adapter/provider payload details may vary, but primary event contract
    remains stable and typed.
- Positive witnesses:
  - Feature 3 Scenario 3.1
  - Feature 3 Scenario 3.2
  - Feature 6 Scenario 6.1
- Falsification witnesses:
  - Feature 1 Scenario 1.3
  - Feature 3 Scenario 3.3
  - Feature 6 Scenario 6.2

### Invariant I3: Credential compatibility across isolated runtimes

- Definition:
  - Runtime identity/credential contracts required for execution are compatible
    across harness and environment isolation boundaries.
  - Missing, incompatible, or unsafe credential posture must fail closed before
    partial execution side effects.
- Positive witnesses:
  - Feature 2 Scenario 2.1
  - Feature 2 Scenario 2.2
  - Feature 5 Scenario 5.1
- Falsification witnesses:
  - Feature 2 Scenario 2.3
  - Feature 4 Scenario 4.2
  - Feature 5 Scenario 5.3

---

## BDD Proof Scenarios

## Feature 1: Harness choice does not change dispatch semantics

### Scenario 1.1: A valid dispatch executes through Harness A
**Given** a dispatch ready for execution
**When** it runs through one supported harness
**Then** the dispatch reaches a coherent terminal outcome
**And** operator-visible execution evidence is produced

### Scenario 1.2: Equivalent dispatch executes through Harness B
**Given** equivalent dispatch intent and inputs
**When** it runs through a different supported harness
**Then** resulting domain semantics are equivalent to Harness A
**And** differences are limited to harness-specific presentation details

### Scenario 1.3: Unsupported harness action is denied safely
**Given** a dispatch requiring a capability the selected harness does not provide
**When** execution is requested
**Then** execution is rejected with a typed capability/compatibility denial
**And** no partial side effects are committed

---

## Feature 2: Environment choice does not change dispatch semantics

### Scenario 2.1: Dispatch executes in one environment type
**Given** a dispatch with valid environment requirements
**When** it executes in one supported environment type
**Then** terminal domain outcome is coherent
**And** execution evidence links to the acquired lease

### Scenario 2.2: Equivalent dispatch executes in a second environment type
**Given** equivalent dispatch intent and inputs
**When** it executes in another supported environment type
**Then** resulting domain semantics are equivalent
**And** differences are limited to environment-specific telemetry details

### Scenario 2.3: Unavailable/incompatible environment is rejected safely
**Given** a dispatch targeting an unavailable or policy-incompatible environment
**When** execution is requested
**Then** the request is rejected with a typed denial/failure reason
**And** no partial execution state is committed

---

## Feature 3: Execution evidence is normalized and safe to persist

### Scenario 3.1: Operators receive normalized lifecycle signals
**Given** executions from different harness and environment combinations
**When** operators inspect progress and terminal state
**Then** lifecycle status classes are consistent across runs
**And** operators do not need adapter-specific decoding

### Scenario 3.2: Adapter-specific failures map to typed Tanren failure classes
**Given** provider/runtime-specific failures during execution
**When** the dispatch fails
**Then** the surfaced failure class is typed and stable
**And** raw provider-only failure formats do not leak as the primary contract

### Scenario 3.3: Sensitive output is redacted before durable persistence
**Given** execution output containing secrets or credential material
**When** evidence is captured and persisted
**Then** sensitive material is redacted in persisted artifacts
**And** no unredacted secret appears in durable event history

---

## Feature 4: Lease lifecycle is safe across all terminal paths

### Scenario 4.1: Success releases environment lease cleanly
**Given** a successful execution
**When** terminal state is reached
**Then** the lease is released/closed deterministically
**And** no orphaned allocation remains

### Scenario 4.2: Failure or cancellation still releases lease cleanly
**Given** a failed or user-cancelled execution
**When** terminal state is reached
**Then** lease cleanup still occurs deterministically
**And** cleanup failure is visible as explicit operational evidence

### Scenario 4.3: Crash recovery prevents orphaned or duplicate execution
**Given** a worker/process crash during active execution
**When** the system recovers
**Then** orphaned leases are reconciled safely
**And** duplicate terminal execution is prevented

---

## Feature 5: Worker execution is durable and policy-driven

### Scenario 5.1: Accepted work is consumed and completed durably
**Given** accepted dispatch work in queue/state storage
**When** worker capacity is available
**Then** work is consumed and progresses to a terminal outcome
**And** terminal evidence remains queryable after process restart

### Scenario 5.2: Retryable failures follow explicit retry policy
**Given** a failure class marked retryable by policy
**When** execution fails
**Then** retry attempts follow configured limits/backoff semantics
**And** retry history is visible to operators

### Scenario 5.3: Non-retryable failures stop without hidden retries
**Given** a failure class marked non-retryable
**When** execution fails
**Then** execution transitions to terminal failure without silent extra retries
**And** remediation is explicit rather than implicit loop behavior

---

## Feature 6: Phase 1 end-to-end reliability can be demonstrated

### Scenario 6.1: Cross-matrix execution demonstrates semantic equivalence
**Given** a representative dispatch matrix
**When** it is executed across at least two harnesses and two environment types
**Then** domain-level outcomes remain semantically equivalent where expected
**And** divergence is reported explicitly when policy/capability requires it

### Scenario 6.2: Proof artifacts are reproducible by another engineer
**Given** documented proof run instructions
**When** another engineer reruns the Phase 1 proof process
**Then** they can reproduce the evidence pack and verdicts
**And** no Phase 1 claim depends on tribal knowledge

---

## Phase 2 Entry Gate (Behavioral)

Move to Phase 2 only when all are true:

1. Every scenario above has both positive and falsification witnesses.
2. Cross-harness and cross-environment equivalence is demonstrably stable.
3. Lease cleanup and crash recovery behavior are reproducible.
4. Remaining gaps are explicitly classified as Phase 2+ scope.

---

## Witness Packaging Requirement

For acceptance, each scenario and each invariant above must map to reproducible
proof artifacts (command logs + verdict JSON) in a Phase 1 proof pack so a
different engineer can re-run and falsify claims without relying on oral
context.
