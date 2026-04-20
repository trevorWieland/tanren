# Phase 0 Proof (BDD)

## Purpose

This document defines how we prove Phase 0 is complete in behavior terms,
not implementation terms.

Audience: technical teammates who are not Tanren specialists.

---

## Phase 0 Story

By the end of Phase 0, Tanren should behave like a reliable **control-plane
foundation**:

- it has a stable, typed model of work and state transitions
- it persists an authoritative event history and can rebuild state from it
- it exposes usable interfaces from one shared contract
- it separates workflow mechanics (code-owned) from agent behavior (markdown-owned)
- it installs and self-hosts its command surface consistently across agent frameworks

Phase 0 is **not** claiming runtime substrate automation, multi-harness execution,
or planner-native orchestration. Those begin in later phases.

---

## Proof Model

Each promise must be proven in two ways:

1. **Positive witness**: the capability works in a realistic flow.
2. **Falsification witness**: an invalid or out-of-scope case is rejected safely.

A promise is accepted only when both witnesses are demonstrated.

---

## BDD Proof Scenarios

## Feature 1: Typed control-plane state is authoritative

### Scenario 1.1: Valid lifecycle changes are accepted
**Given** a new project state
**When** a valid sequence of lifecycle mutations is submitted
**Then** state advances predictably according to declared transition rules
**And** resulting status is consistent across query surfaces

Proof evidence:
- lifecycle trace with expected transitions
- matching read-model outputs from the same state

### Scenario 1.2: Invalid lifecycle changes are rejected
**Given** an entity already in a terminal or incompatible state
**When** an illegal transition is attempted
**Then** the operation is rejected with a typed error
**And** no partial state mutation is persisted

Proof evidence:
- typed error payload
- unchanged persisted state after rejection

---

## Feature 2: Event history is durable, transactional, and replayable

### Scenario 2.1: Accepted changes emit durable events
**Given** a successful mutation
**When** the mutation commits
**Then** its event(s) are appended durably
**And** projections/read-models reflect the same committed truth

Proof evidence:
- committed event history entries
- matching projection snapshots

### Scenario 2.2: Replay reconstructs equivalent state
**Given** a committed event history
**When** state is rebuilt from replay into a clean store
**Then** reconstructed state matches the original operational state

Proof evidence:
- source vs replayed state comparison report

### Scenario 2.3: Corrupt/non-canonical history fails safely
**Given** malformed or semantically invalid event input
**When** replay is attempted
**Then** replay fails with explicit diagnostics
**And** no partial replay is left behind

Proof evidence:
- failure report with location/cause
- atomic rollback confirmation

---

## Feature 3: At least one contract-derived interface is operational

### Scenario 3.1: Operator can run core dispatch flow end-to-end
**Given** a configured environment and valid actor identity
**When** an operator performs create, inspect, list, and cancel flows
**Then** each action is accepted/rejected according to policy and lifecycle rules
**And** observed outcomes match the same domain contract semantics

Proof evidence:
- end-to-end session transcript
- correlated state and event outputs

### Scenario 3.2: Authentication and replay protections are enforced
**Given** invalid identity material or replayed mutation credentials
**When** a protected command is attempted
**Then** the command is rejected with typed auth/replay errors
**And** no unintended mutation occurs

Proof evidence:
- rejection traces for invalid identity and replay attempts

---

## Feature 4: Methodology boundary is explicit and enforced

### Scenario 4.1: Structured workflow state is code-owned
**Given** an agent phase operating on a spec
**When** structured state must change (tasks, findings, scores, outcomes)
**Then** mutation occurs only through typed tools
**And** orchestrator-owned artifacts are not directly agent-edited

Proof evidence:
- tool-call-driven mutation trace
- no direct structured-file-write path accepted

### Scenario 4.2: Agent markdown remains behavior-only
**Given** installed command assets
**When** command content is inspected
**Then** commands describe behavior and required tool use
**And** they do not embed workflow-mechanics ownership responsibilities

Proof evidence:
- command audit report against boundary rules

---

## Feature 5: Task completion is monotonic and guard-based

### Scenario 5.1: Completion requires required guards
**Given** a task marked implemented
**When** required guards are satisfied in any order
**Then** completion occurs only after all required guards converge

Proof evidence:
- guard convergence timeline
- terminal completion event only at convergence

### Scenario 5.2: Terminal tasks are not reopened
**Given** a completed task
**When** remediation is needed
**Then** remediation is represented as a new task (not reopening the completed one)

Proof evidence:
- completed task remains terminal
- new remediation task with explicit origin/provenance

---

## Feature 6: Tool surface is typed, scoped, and transport-consistent

### Scenario 6.1: Inputs are validated at the boundary
**Given** malformed tool input
**When** the tool is invoked
**Then** it returns a typed validation error
**And** no side effect occurs

Proof evidence:
- typed validation response
- unchanged state/event history

### Scenario 6.2: Capability scoping prevents out-of-phase actions
**Given** a phase with a bounded capability set
**When** it attempts an out-of-scope tool action
**Then** the call is denied with `CapabilityDenied`

Proof evidence:
- denied call trace
- no unauthorized mutation

### Scenario 6.3: MCP and CLI produce equivalent semantics
**Given** the same valid request semantics
**When** executed through MCP and CLI transports
**Then** resulting domain effects are equivalent

Proof evidence:
- transport parity comparison report

---

## Feature 7: Installer and self-hosting are deterministic

### Scenario 7.1: Install output is predictable and idempotent
**Given** a configured repository
**When** install is run repeatedly without source/config change
**Then** first run renders targets and later runs are no-op

Proof evidence:
- initial render report
- repeat-run no-drift report

### Scenario 7.2: Drift is detectable and explicit
**Given** rendered outputs diverge from source-of-truth templates
**When** strict dry-run is executed
**Then** drift is reported and process fails explicitly

Proof evidence:
- drift detection report with non-success status

### Scenario 7.3: Multi-agent targets stay semantically aligned
**Given** shared command source
**When** artifacts are rendered for multiple frameworks
**Then** framework-specific wrappers may differ
**And** command intent/capability semantics remain equivalent

Proof evidence:
- cross-target semantic parity report

---

## Feature 8: Manual methodology walkthrough is possible now

### Scenario 8.1: Human-guided end-to-end spec loop runs in Phase 0
**Given** a new spec in manual self-hosting mode
**When** the 7-step sequence is performed
**Then** structured outputs flow through typed tools
**And** orchestrator progress/state remains coherent through the loop

Proof evidence:
- one complete manual walkthrough artifact set
- consistent task/finding/progress trace from start to walk-spec

---

## Phase 1 Entry Gate (What We Must Be Comfortable With)

We should only move to Phase 1 when all are true:

1. Every feature above has both a positive and falsification witness.
2. Proof artifacts are reproducible by another engineer using documented steps.
3. No Phase 0 claim depends on implicit behavior or tribal knowledge.
4. Remaining gaps are explicitly marked as Phase 1+ scope, not hidden defects.

---

## Suggested Proof Review Cadence

1. Review this BDD document and agree on wording of each promise.
2. Map each scenario to a concrete proof artifact owner.
3. Run a proof day and collect artifacts in one evidence pack.
4. Hold a Phase 0 exit review using this checklist only.

