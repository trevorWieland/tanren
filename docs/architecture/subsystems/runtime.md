---
schema: tanren.subsystem_architecture.v0
subsystem: runtime
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Runtime Architecture

## Purpose

This document defines Tanren's execution runtime architecture. Runtime is the
execution substrate that safely runs agentic phases and automated gates in
isolated environments.

Runtime does not decide what work exists or what state transition should happen
next. Orchestration decides the active spec flow. Runtime decides where an
assignment runs, how the execution target is prepared, how the harness or gate
is invoked, how access is scoped, how failures are classified, and how the
target is recovered or destroyed.

## Subsystem Boundary

The runtime subsystem owns:

- execution assignments;
- execution workers and runner processes;
- execution queues, claims, and leases;
- execution placement policy inputs and placement decisions;
- execution target provisioning and teardown;
- workspace preparation for Tanren-managed spec branches;
- harness invocation inside execution targets;
- automated gate invocation inside execution targets;
- worker-scoped temporary access delivery;
- runtime output redaction and bounded result reporting;
- cancellation, timeout, retry, and recovery behavior;
- execution target health, cleanup, and reconciliation.

The runtime subsystem does not own planning, roadmap sequencing, spec or task
lifecycle meaning, behavior-proof semantics, source-control provider APIs,
issue trackers, webhooks, projection workers, secret storage, permission grant
models, or observation dashboards.

## Runtime Worker Distinction

Tanren uses "worker" in two different operational senses. They must remain
distinct.

- **Control-plane workers** are Tanren service-stack processes that run
  projections, scheduling, outbox delivery, webhook delivery, cleanup,
  reconciliation, and other internal background work. They are part of the
  control plane.
- **Execution workers** are isolated assignment runners that execute agentic
  phases and automated gates inside provisioned execution targets. They are
  part of the execution substrate.

This document owns execution workers. Control-plane worker behavior belongs to
state, integrations, operations, and the owning subsystem for the work being
performed.

## Core Invariants

1. **Execution is isolated.** Agentic work and automated gates run in local
   containers, remote containers, remote VMs, or equivalent isolated execution
   targets.
2. **No unmanaged local worktrees.** Tanren does not run product execution in
   an unmanaged host worktree as a core runtime path.
3. **Targets are sandboxed and disposable.** Agents may perform destructive
   actions inside targets. Reused or pooled targets must be resettable and
   treated as destructive sandboxes.
4. **Default execution is simple and safe.** The default strategy is one
   execution target with sequential phase execution.
5. **Parallelism is policy-controlled.** Faster strategies are supported, but
   they are configuration and cost/performance choices, not different
   orchestration semantics.
6. **Harnesses run inside the target.** Codex, Claude Code, OpenCode, and
   future harness adapters execute from within the assigned environment where
   possible.
7. **Automated gates run in the target.** Gate commands run in the same
   execution environment class as agentic phases for parity.
8. **Targets communicate through public contracts.** Execution targets talk to
   Tanren only through authenticated API or MCP. They never get direct database
   access.
9. **Provider credentials stay in the control plane.** Cloud, VM, and provider
   credentials used to provision targets are not placed in the target.
10. **Worker access is assignment-scoped.** Targets receive only the
    credentials, API/MCP capabilities, and provider access required for the
    assignment.
11. **Runtime emits bounded results.** Runtime does not persist unbounded logs
    by default. It emits redacted result events and leaves code changes in the
    Tanren-managed spec branch.
12. **Recovery is reconciliation-based.** Expired leases, interrupted sessions,
    orphaned targets, failed cleanup, and duplicate claims are repaired from
    durable state.

## Assignment Model

An execution assignment is a bounded request to run one phase, gate, or runtime
operation.

Assignment metadata includes:

- assignment ID;
- project ID;
- spec ID;
- task ID when task-scoped;
- phase or gate key;
- required harness or runner class;
- source branch and base branch;
- execution strategy;
- placement constraints;
- required capabilities;
- scoped credential references;
- timeout and retry policy;
- expected result shape;
- correlation and causation IDs.

Assignments are created by orchestration, assessment, operations, or other
control-plane subsystems. Runtime executes them and reports outcomes. Runtime
does not reinterpret product or spec semantics.

## Queue And Lease Model

Runtime uses durable Postgres-backed queues and leases.

Queue and lease rules:

- assignments are durable records;
- execution workers claim assignments transactionally;
- each active assignment has a lease;
- leases expire if workers stop heartbeating;
- expired assignments are reconciled before retry or reassignment;
- duplicate visible execution is avoided through idempotency and assignment
  state;
- cancellation marks assignments and revokes scoped access;
- retry records distinguish infrastructure retry from semantic repair.

Postgres wakeups may notify workers, but durable queue and lease state is the
source of truth.

## Execution Target Classes

Supported target classes include:

- local container;
- remote container;
- remote VM;
- pooled destructive target.

The baseline local/team profile uses containers. Remote VM or remote container
targets are selected by placement policy when isolation, scale, hardware,
network location, or organization governance requires them.

Pooled targets are allowed only when they are treated as destructive sandboxes.
They must be reset between assignments according to policy. They are not
general-purpose long-lived infrastructure that agents carefully preserve.

### Deployment Posture And Runtime Capabilities

Runtime capabilities are gated by the deployment posture. The runtime subsystem
must respect the active posture when evaluating placement policy and provisioning
execution targets.

Posture-runtime rules:

- `hosted`: all execution target classes are available. Remote execution,
  parallel strategies, and cloud/VM provider targets are fully supported.
- `self_hosted`: all execution target classes are available. The operator
  manages provider credentials and infrastructure.
- `local_only`: only `local_container` target class is available. Remote
  containers, remote VMs, and pooled destructive targets are unavailable. The
  runtime must reject placement requests for unavailable target classes and
  report the posture as the reason.

The `RuntimeCapabilityView` contract type describes available target classes,
remote execution support, and parallelism constraints under the active posture.
Runtime workers and placement policy consumers must consult this view before
provisioning.

## Execution Strategy

Runtime supports these execution-target strategies:

- **shared_serial**: one target for the spec; phases run sequentially.
- **shared_parallel**: one target for the spec; safe batch members may run
  concurrently in the same target.
- **per_spec**: one target per spec, retained across phases according to
  policy.
- **per_task**: one target per task or task repair loop.
- **per_phase**: one target per phase or gate.
- **per_batch_clone**: clone or fork equivalent targets for parallel batch
  checks.

The default strategy is `shared_serial`. It minimizes concurrency surprises and
is the safest baseline for self-hosted users. Faster strategies are
cost/performance optimizations that operators can enable after validating their
project and provider setup.

The orchestration state machine is invariant across strategies. Placement
strategy affects cost, speed, isolation, and debug ergonomics, not the required
phase order or acceptance rules.

## Workspace Preparation

Each spec uses one Tanren-managed source-control branch.

Workspace rules:

- runtime prepares targets by cloning the provider-backed repository branch;
- one branch is created and managed per spec;
- runtime checks out the correct base and spec branch;
- mutating units of work commit changes to the spec branch;
- commits are created at meaningful unit-of-work boundaries;
- branch identity is reported back to orchestration and integrations;
- source-control provider APIs are used through integrations, not directly by
  arbitrary target code.

Provider-cloned branches are the baseline workspace model. Archive injection or
other optimizations may exist later, but they must preserve branch identity,
commit provenance, and recovery semantics.

## Target Provisioning And Ownership

Tanren control-plane code owns target provisioning, bootstrap, communication,
and teardown.

Provisioning rules:

- provider credentials remain in the control plane;
- targets receive no cloud or VM provider keys unless explicitly required by
  the assignment and policy;
- bootstrap installs required runtime dependencies, harnesses, API/MCP access,
  and project tools;
- target health and readiness are verified before assignment execution;
- teardown or retention follows runtime policy.

This prevents an agent running inside a VM from needing the provider API key
that created the VM.

## Harness Adapter Contract

Harnesses are adapters. The required adapter families are Codex, Claude Code,
and OpenCode.

Harness adapter rules:

- adapters declare capabilities before execution;
- runtime performs capability preflight before side effects;
- adapters receive normalized execution requests;
- adapters run inside the execution target where possible;
- adapter output is redacted before persistence;
- provider or harness failures are normalized into stable runtime failure
  classes;
- provider metadata is sanitized and fail-closed before exposure;
- adapters declare whether session resume is supported;
- conformance proof verifies preflight, redaction, failure mapping, metadata
  sanitization, and deterministic terminal semantics.

Adapters do not decide assignment scope, product semantics, or permission
policy. They execute the assignment runtime gives them.

## Automated Gates

Automated gates run inside execution targets.

Gate rules:

- `task-gate` and `spec-gate` execute in the prepared workspace;
- gate commands are resolved from event-sourced configuration;
- gate output is bounded and redacted before reporting;
- gate pass/fail results are reported as runtime outcomes;
- orchestration interprets gate outcomes in task or candidate validation
  batches.

Running gates in the same execution environment preserves parity between agent
work and verification.

## Worker-Scoped Access

Runtime obtains scoped access from identity-policy and configuration-secrets.

Access rules:

- assignment credentials are scoped to one assignment or bounded workflow;
- API and MCP credentials include actor, scope, capabilities, expiration, and
  correlation metadata;
- provider credentials are delivered only as scoped references or operation
  permissions where possible;
- targets receive only what the phase needs;
- cancellation, completion, expiration, or policy change revokes access.

Worker-scoped access supports both internal Tanren workers and configured
builder-owned agent clients without broad standing credentials.

## Communication Contract

Execution targets communicate with Tanren through authenticated API or MCP.

Rules:

- no direct database access from targets;
- all state mutation uses typed commands;
- all reads use permitted queries or tools;
- idempotency keys are used for retryable mutation;
- target requests carry assignment identity and correlation metadata;
- capability discovery and enforcement remain server-side;
- secret values are never logged or emitted in protocol payloads.

HTTP MCP is the product transport for agent/tool access. API is used where a
runtime client or gate runner needs general machine contracts.

## Output And Result Handling

Runtime persists bounded, redacted result summaries rather than full logs by
default.

Runtime result records include:

- assignment outcome;
- failure class where applicable;
- phase or gate key;
- target identity;
- branch and commit references;
- bounded stdout/stderr tails where policy allows;
- redaction verdict;
- proof output references where applicable;
- timing and resource metadata;
- retry or recovery metadata.

Raw logs may have short operational retention if configured. Durable product
state should rely on events, commits, behavior proof, assessment results, and
bounded runtime summaries rather than an unbounded log archive.

## Failure Taxonomy

Runtime normalizes failures into stable classes consumed by orchestration,
assessment, observation, and operations.

Failure classes include:

- capability denial;
- policy denial;
- credential unavailable or revoked;
- target provisioning failure;
- target bootstrap failure;
- target health failure;
- harness unavailable;
- harness terminal failure;
- gate failure;
- timeout;
- cancellation;
- lease lost;
- provider failure;
- network failure;
- redaction failure;
- cleanup failure;
- unknown infrastructure failure.

Semantic product or implementation failures are routed to orchestration
investigation. Infrastructure failures may be retried by runtime policy.

## Cancellation, Retry, And Recovery

Runtime handles infrastructure retry and execution recovery.

Runtime may retry:

- transient provider failures;
- network interruptions;
- target provisioning failures;
- lost workers before side effects;
- interrupted resumable harness sessions;
- cleanup failures.

Runtime does not repair semantic failures. If a gate fails, an audit finds a
quality issue, adherence finds a standards violation, or behavior proof fails,
orchestration decides the repair loop.

Resumability is adapter capability-driven. Runtime resumes sessions when the
adapter and target support it. Otherwise interrupted assignments reconcile into
retry, restart, reassignment, or escalation.

## Target Retention And Cleanup

Target retention is configurable.

Supported retention policies include:

- destroy immediately on success;
- destroy after inactivity timeout;
- retain on failure for a bounded debugging window;
- retain until manual release;
- return to destructive resettable pool.

The default is to destroy after a configured inactivity or pending window.
Targets retained for debugging still have assignment credentials revoked or
expired according to policy.

Cleanup includes:

- revoking worker-scoped access;
- stopping harness sessions;
- removing or resetting execution targets;
- collecting bounded runtime summaries;
- reconciling branch and commit state;
- marking cleanup outcome.

Failed cleanup is durable operational state and must be retried or surfaced.

## Health And Reconciliation

Runtime continuously reconciles execution state.

Reconciliation detects:

- expired leases;
- missing heartbeats;
- orphaned targets;
- assignments running after cancellation;
- targets retained past policy;
- failed cleanup;
- duplicate claims;
- branch or commit mismatch;
- provider target drift;
- unavailable harnesses;
- stale worker capability reports.

Reconciliation emits events and repair work. Operators can observe runtime
health through observation and operations views.

## Audit And Events

Runtime state is event-sourced. Events include:

- assignment queued, claimed, started, heartbeated, completed, failed,
  cancelled, retried, or reconciled;
- lease acquired, renewed, expired, or released;
- target selected, provisioned, bootstrapped, healthy, unhealthy, retained,
  destroyed, or cleanup-failed;
- workspace cloned, branch checked out, commit created, or branch mismatch
  detected;
- harness capability reported, preflight denied, started, failed, resumed, or
  completed;
- gate started, passed, failed, timed out, or cancelled;
- worker-scoped access granted, used, expired, or revoked;
- output redacted, truncated, accepted, or rejected;
- runtime failure classified.

Runtime events never include secret values or unbounded logs.

## Accepted Runtime Decisions

- Runtime owns execution substrate, not orchestration semantics.
- Control-plane workers and execution workers are distinct categories.
- Agentic phases and automated gates run inside execution targets.
- Default execution strategy is `shared_serial`.
- Runtime supports `shared_serial`, `shared_parallel`, `per_spec`,
  `per_task`, `per_phase`, and `per_batch_clone` strategies.
- Containers are the baseline target class; remote containers and remote VMs
  are supported by placement policy.
- Execution targets are sandboxed and disposable.
- Pooled targets must be destructive and resettable.
- Tanren control plane owns provisioning, bootstrap, communication, and
  teardown.
- Provider credentials for target provisioning stay in the control plane.
- Harnesses run inside execution targets where possible.
- Codex, Claude Code, and OpenCode are required harness adapter families.
- Execution targets communicate with Tanren only through authenticated API or
  MCP.
- Workspaces are provider-cloned branches, one Tanren-managed branch per spec.
- Runtime commits at meaningful mutating unit-of-work boundaries.
- Runtime persists bounded, redacted summaries rather than full logs by
  default.
- Runtime handles infrastructure retry; orchestration handles semantic repair.
- Resumability is adapter capability-driven and not universally required.
- Execution strategy configuration supports `shared_serial`,
  `shared_parallel`, `cloned_parallel`, and `remote_pool`, with
  `shared_serial` as the default.
- Task check batches may use `shared_parallel` only when the configured gates
  are read-only or declare non-overlapping mutable paths.
- The default inactivity timeout before target destruction is 30 minutes.
- Runtime commits after each mutating task, repair task, and spec-level repair
  loop that changes the workspace.
- Automatic retry applies to provisioning failure, transient provider failure,
  network interruption, target heartbeat loss, and retryable harness startup
  failure.
- Harness target bootstrap installs the selected harness assets, MCP/API
  connection config, scoped source-control credentials, assignment metadata,
  redaction policy, and runtime reporting hooks.

## Rejected Alternatives

- **Unmanaged local worktree execution.** Rejected because it weakens
  isolation, cleanup, credentials, and repeatability.
- **Single hard-coded execution strategy.** Rejected because projects and
  operators need explicit cost/performance/isolation tradeoffs.
- **Maximum parallelism by default.** Rejected because the safest baseline is a
  single sequential target; users can opt into faster strategies.
- **Provider credentials inside execution targets by default.** Rejected
  because target provisioning should be controlled by the Tanren control plane.
- **Direct database access from targets.** Rejected because all execution
  communication must go through authenticated API or MCP contracts.
- **Unbounded runtime log retention.** Rejected because durable product state
  should be structured events, commits, proof, assessment, and bounded
  summaries.
- **Harness-specific orchestration behavior.** Rejected because harnesses are
  adapters underneath Tanren's assignment and event contracts.
