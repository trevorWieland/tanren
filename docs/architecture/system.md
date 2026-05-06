---
schema: tanren.system_architecture.v0
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# System Architecture

## Purpose

This document is the top-level architecture record for Tanren. It defines the
system boundary, durable invariants, deployment posture, state model, interface
model, execution model, and subsystem ownership rules that every other
architecture record must follow.

Tanren is a self-hosted, product-to-proof control plane for agentic software
delivery. It owns product intent, accepted behavior contracts, architecture
decisions, implementation assessment, roadmap graph, specs, orchestration,
policy, runtime placement, worker assignments, behavior proof, quality
controls, review, and release learning.

Agent runtimes sit below Tanren. They execute bounded assignments inside
Tanren-defined scope. They do not decide what work exists, what is allowed,
what behavior proof is sufficient, or when work counts as complete.

Human and integration clients sit above Tanren. CLI, API, MCP, TUI, and
responsive web UI surfaces must expose coherent state because they all operate
against the same event log, command contracts, policy checks, and read models.

## System Boundary

```text
Human and client surfaces
  CLI | TUI | responsive web UI | API clients | MCP clients

Application contracts
  commands | queries | validation errors | subscriptions | idempotency

Tanren control-plane services
  planning | behavior catalog | architecture | implementation assessment
  roadmap | orchestration | identity and policy | configuration and secrets
  runtime placement | provider integrations | client integrations
  observation | behavior proof | quality controls

Durable state
  append-only event log | projections | read models | audit records
  repo-local generated artifacts

Execution substrate
  workers | queues | leases | containers | remote execution targets
  harness adapters | provider adapters

External systems
  source control | CI | issue trackers | cloud/VM providers
  agent CLIs | webhook consumers | notification channels
```

Tanren is not an agent runtime, editor, ticket tracker, CI system, generic task
runner, or SaaS-specific application. It is the control plane that preserves
product intent, governs work, dispatches bounded execution, and proves outcomes.

## System Invariants

1. **Behavior is the unit of product meaning.** Product planning, roadmap work,
   specs, behavior proof, assertions, and release learning must remain
   traceable to accepted behavior contracts.
2. **The event log is primary canon.** Every durable state transition is an
   event. Read models, relational tables, repo documents, spec files, and
   generated artifacts are projections from the event stream.
3. **Tanren-owned projections are not source of truth.** Manual edits to
   Tanren-owned generated artifacts are drift. Tanren should prevent drift where
   practical, detect it when it occurs, and recreate projections from canonical
   events when needed.
4. **All public surfaces share one contract model.** CLI, API, MCP, TUI, and
   responsive web UI must agree on resource identity, validation errors,
   permission denials, idempotency behavior, freshness, and redaction.
5. **Solo and team use share one data model.** A solo install still has real
   account, project, organization, permission, policy, service-account, and
   audit records. First-run setup may bootstrap simple defaults, but it must not
   create a separate local-only architecture.
6. **One project is exactly one source-control repository.** Monorepos are
   supported inside one project. Polyrepo work is represented through multiple
   projects and explicit cross-project dependencies.
7. **Execution is isolated by default.** Agent work runs in containers or remote
   execution targets governed by placement policy, leases, credentials, and
   proof obligations. Unmanaged local worktree execution is not a product
   architecture path.
8. **Harnesses are adapters; method phases are core.** Codex, Claude Code, and
   OpenCode are harness adapters for executing Tanren assignments. Work-unit
   phases such as `do-task`, `audit-spec`, and `walk-spec` are core product
   method primitives, not provider-specific plugin behavior.
9. **Governance is core, not enterprise garnish.** Accounts, organizations,
   permissions, approvals, service accounts, API keys, credential policy, and
   audit history exist because solo and team builders both need safe automation.
10. **Observation is a first-class control-plane subsystem.** Dashboards,
    status summaries, digests, reports, trends, forecasts, provenance, bounds,
    and proof/source links are derived from canonical events and read models,
    not from ad hoc interface-local state.

## Deployment Posture

Tanren's architecture is self-hosted open-source infrastructure. Local use,
single-node team use, and distributed team use are installation profiles of the
same containerized system.

The primary packaging baseline is Docker Compose because it gives local and
team installs a repeatable control-plane, Postgres, worker, and network shape.
The architecture must not depend on Docker Compose specifically: equivalent
container orchestration through Podman, Kubernetes, Nomad, or another operator
choice may run the same images, environment contracts, volumes, and service
interfaces.

There is no separate Tanren distinction between self-hosted and hosted from the
system architecture's point of view. A managed premium offering can run the same
self-hosted Tanren stack with additional external layers for billing, marketing,
support operations, workspace scaling, and cost optimization. Those commercial
layers are outside this repository's architecture.

### User-Facing Posture Decision

Although the system architecture is uniform, the user chooses a **deployment
posture** that determines which capabilities are available. The posture is a
top-level decision made during first-run setup and gates subsequent provider
selection, runtime configuration, and project creation.

Supported postures:

| Posture | Description |
|---------|-------------|
| `hosted` | Tanren operates as a managed hosted service. All capabilities are available; operational overhead is managed externally. |
| `self_hosted` | Tanren operates as self-hosted infrastructure. All local and remote capabilities are available; the operator manages infrastructure and operations. |
| `local_only` | Tanren operates in a local-only mode with reduced capabilities. Remote execution, cloud providers, external secret stores, and team collaboration are unavailable. Suitable for individual evaluation and development. |

Posture rules:

- The posture is visible to the user before project work is dispatched.
- Tanren explains which capabilities are available or unavailable for the
  chosen posture through capability summaries with user-readable reasons for
  each unavailable category.
- Later runtime and credential choices inherit the selected posture unless a
  user with permission explicitly changes it.
- The posture is stored as domain state; the contract types in
  `tanren-contract` define the wire shapes for selection requests, posture
  views, capability summaries, runtime capability views, credential capability
  views, and posture failure codes.
- Selection requests carry raw posture input (an unvalidated string) so that
  unsupported values reach shared service validation and return
  `unsupported_posture` uniformly, rather than being rejected differently by
  each interface's deserialization layer.

### Capability Matrix

The posture determines availability of capability categories aligned with
subsystem boundaries:

| Category | `hosted` | `self_hosted` | `local_only` |
|----------|----------|---------------|--------------|
| Execution targets | all | all | local containers only |
| Harness adapters | all | all | all |
| Remote providers | available | available | unavailable |
| Team collaboration | available | available | unavailable |
| External secret stores | available | available | unavailable |
| Cloud credentials | available | available | unavailable |
| Remote proof | available | available | unavailable |
| Webhook integrations | available | available | unavailable |
| Provider integrations | all | all | local only |
| Service accounts | available | available | unavailable |

Capability summaries returned by the posture view include user-readable reasons
for each unavailable category. For example, `local_only` explains that remote
providers are unavailable because the posture does not configure outbound cloud
or VM provider access.

Every install profile contains the same conceptual components:

- a control-plane API service;
- a responsive web UI served against the API;
- an MCP service for agent/tool clients;
- CLI and TUI clients for local and operator workflows;
- a daemon or scheduler process for background and event-triggered work;
- Postgres for canonical events, projections, read models, and audit records;
- worker services that execute assignments through harness adapters;
- container or remote execution targets for isolated work;
- provider integration adapters for external systems;
- provider action, notification, webhook delivery, and projection workers.

First-run setup creates or selects a bootstrap account and grants the initial
administrative permissions needed to configure the installation. The same
account, organization, project, policy, and audit model remains in force when a
solo user later adds teammates.

## Durable State Model

The append-only event log is Tanren's canonical mutation history. Every durable
change is represented as a typed event, including planning edits, behavior
changes, architecture decisions, implementation assessments, roadmap revisions,
spec lifecycle changes, task updates, policy changes, credential metadata
changes, provider connection changes, worker assignments, approvals, behavior
proof updates, observation snapshots, release outcomes, and projection drift.

Postgres is the intended durable state substrate. Relational tables and
materialized read models are projections from the event log, optimized for
queries, filtering, subscriptions, reports, and interface rendering. They are
not independent sources of truth.

Repo-local files remain important because they make Tanren state visible inside
the repository where builders and agents work. Product docs, behavior docs,
architecture docs, roadmap views, specs, task files, proof files, generated
agent commands, and standards profiles are generated or validated projections
owned by Tanren unless explicitly classified as user-owned input.

Projection rules:

- Tanren-owned projections are regenerated from events.
- Manual edits to Tanren-owned projections are drift.
- Drift is surfaced clearly and remediated by recreating the projection from
  canonical events.
- Actions that depend on a drifted projection must not silently continue from
  the edited file.
- User-owned input files may be imported through typed Tanren commands that
  produce reviewed events.

## Interface Model

Tanren has five first-class public surfaces:

- **Responsive web UI**: the primary human surface across desktop and phone.
  It presents planning, status, observation, review, approvals, configuration,
  integrations, and operational workflows through API-backed read models and
  commands.
- **CLI**: a local, scriptable, operator-friendly interface for installation,
  administration, project workflows, and automation.
- **TUI**: a terminal interface for live operation, observation, worker/queue
  status, and focused control-plane workflows.
- **API**: the stable machine-client contract for web UI, external automation,
  CI/source-control reporting, provider integrations, webhooks, subscriptions,
  idempotent commands, and read models.
- **MCP**: the agent-facing tool interface used by harnesses and LLM clients to
  mutate and observe Tanren state within scoped capabilities.

The API is the general integration contract. MCP is an agent tool contract. CLI
and TUI are human/operator clients. The web UI is a first-class product surface,
not a documentation or administration afterthought.

All interfaces must share:

- stable resource identifiers;
- typed command and query contracts;
- consistent validation and policy error categories;
- idempotency behavior for mutation requests;
- permission and redaction semantics;
- freshness/cursor metadata for read models and subscriptions;
- links from summaries to supporting proof or source signals where visible;
- explicit unsupported-action responses instead of context loss.

## Control-Plane Subsystems

Tanren is organized around these subsystem ownership boundaries.

### Planning

Owns product mission, target users, problems, success signals, behavior
catalog, architecture records, implementation assessment, roadmap graph,
planning proposals, decision memory, assumptions, rejected alternatives, and
planning-change review.

Planning state is typed event-sourced state with repo-local markdown and JSON
projections. Planning records must preserve history and distinguish accepted
direction from proposals, drafts, stale assumptions, and rejected alternatives.

### Orchestration

Owns spec lifecycle, task lifecycle, phase execution, control-batch
scheduling, walks, review feedback routing, merge routing, and cleanup
transitions.

Orchestration turns roadmap graph nodes into delivered and walked behavior. It
does not own product intent or quality-control semantics; it consumes accepted
behavior, shaped spec state, and quality-control results.

### Quality Controls

Owns automated gates, audit rubric checks, standards adherence checks,
run-demo critique semantics, task/spec guard semantics, active-spec quality
finding routing, and quality-control configuration.

Quality controls determine whether active work is acceptable enough to
progress. Orchestration owns when controls run and how their results move the
spec lifecycle.

### Runtime

Owns worker assignment, queues, leases, placement policy evaluation, execution
environment selection, cancellation, retry, recovery, reconciliation, progress
reporting, worker-scoped temporary access, and cleanup.

Runtime work executes in containers or remote execution targets. Docker-based
local execution and remote/VM-backed execution are both placement targets under
the same policy, proof, and cleanup model.

### Harness Adapters

Own normalized execution against supported agent CLIs. Codex, Claude Code, and
OpenCode are required adapter families. Harness adapters report capability,
execute assigned phases, normalize failures, redact output, report proof
outputs, and remain subordinate to Tanren's assignment and event contracts.

### Identity And Policy

Owns accounts, organizations, projects, memberships, invitations, roles as
permission templates, direct permission grants, service accounts, API keys,
approval policy, runtime placement policy, budget/quota policy, standards
policy, credential-use policy, and policy-denial explanations.

Policy checks apply uniformly across solo and team installs. Local bootstrap
may create permissive defaults, but it must still use the same records and
events.

### Configuration And Secrets

Owns user-tier, account-tier, project-tier, and organization-tier
configuration; effective configuration resolution; inheritance and override
rules; credential and secret metadata; rotation; revocation; stale or
overscoped detection; secret usage views; and configuration history.

Secret values must never appear in event payloads, projections, logs, reports,
or proof artifacts. Events and read models may record secret presence,
scope, owner class, version, usage, and lifecycle metadata.

### Provider Integrations

Own Tanren's connections to external systems that Tanren calls: source control,
CI, issue trackers, cloud/VM providers, identity providers, notification
channels, and other provider APIs.

Provider integrations track ownership mode, reachable resources, provider
permissions, health, authorization failure, external action audit, and
provider-specific source signals behind Tanren-level status.

### Client Integrations

Own external systems calling Tanren through API, webhooks, subscriptions, API
keys, service accounts, idempotent requests, schema version negotiation, replay,
rate limits, and backpressure reporting.

Provider integrations and client integrations may involve the same external
platform, but they are distinct architectural roles: one is Tanren calling out;
the other is external automation calling Tanren.

### Observation

Owns dashboards, project overview, work pipeline, quality signals, health
signals, time-window comparison, forecasts, risk summaries, provenance-aware
status, reports, observer digests, recently shipped outcomes, post-release
health, change summaries, and cross-project dependency risk.

Observation is derived from event streams, read models, behavior proof,
assessment results, and source links. Observation claims must distinguish
direct proof, source signals, inference, missing data, stale data, hidden data,
and redacted data.

### Behavior Proof

Owns behavior-to-proof linkage, BDD feature expectations, positive and
falsification witnesses, proof run status, proof-quality signals, coverage
interpretation, and mutation-testing interpretation for behavior proof.

Behavior proof supports assessment classification. It is not a generic artifact
vault and does not own raw log retention, CI artifact archiving, or provider
payload storage.

### Delivery And Operations

Delivery owns install, packaging, generated command assets, standards profile
installation, MCP configuration, container image contracts, Compose baseline,
upgrade, uninstall, and projection drift checks.

Operations owns backup/export, restore, disaster-recovery validation,
maintenance, incident, and safe modes, worker/queue/target health,
pause/resume/drain controls by scope, cost and quota enforcement, schedules,
operational audit export, and production readiness of the self-hosted stack.

## Per-Binary Library Crate Pattern

Every interface binary (`tanren-api`, `tanren-cli`, `tanren-mcp`,
`tanren-tui`) is implemented as a paired pair: a **per-binary library
crate** at `crates/tanren-{api,cli,mcp,tui}-app/` that owns all of the
binary's logic, and a **thin binary shell** at
`bin/tanren-{api,cli,mcp,tui}/src/main.rs` (≤ 50 lines) that parses CLI
flags, calls `tanren_observability::init`, and hands off to the lib
crate's entry point.

This pattern is required because Cargo binary crates cannot be depended
on by integration test crates. Promoting the logic into a real lib crate
lets BDD wire harnesses (in `tanren-testkit`) depend on the same code
the binary runs — `@api` scenarios spin up `tanren-api-app` directly on
an ephemeral port; `@mcp` scenarios spin up `tanren-mcp-app` the same
way. The binary subprocess is exercised separately by `@cli` and `@tui`
scenarios, which need real process boundaries (`tokio::process::Command`
and pty respectively).

The 50-line cap is mechanically enforced by `just check-thin-binary`.
Profile cross-links: [thin-binary-crate](../../profiles/rust-cargo/architecture/thin-binary-crate.md)
and [crate-layering](../../profiles/rust-cargo/architecture/crate-layering.md).

## Behavior Coverage Ownership

Every accepted behavior area must have one primary architecture owner:

| Behavior area | Primary owner |
|---|---|
| `product-discovery` | Planning |
| `product-planning` | Planning |
| `architecture-planning` | Planning |
| `implementation-assessment` | Assessment |
| `prioritization` | Planning |
| `intake` | Assessment |
| `repo-understanding` | Planning |
| `standards-evolution` | Planning |
| `spec-lifecycle` | Orchestration |
| `spec-quality` | Quality Controls |
| `implementation-loop` | Orchestration |
| `runtime-substrate` | Runtime |
| `runtime-actor-contract` | Runtime |
| `autonomy-controls` | Identity And Policy |
| `planner-orchestration` | Planning |
| `review-merge` | Orchestration |
| `walk-acceptance` | Orchestration |
| `behavior-proof` | Behavior Proof |
| `decision-memory` | Planning |
| `undo-recovery` | Planning |
| `release-learning` | Assessment |
| `findings` | Assessment |
| `team-coordination` | Orchestration |
| `governance` | Identity And Policy |
| `configuration` | Configuration And Secrets |
| `integration-management` | Provider Integrations |
| `integration-contract` | Client Integrations |
| `external-tracker` | Provider Integrations |
| `observation` | Observation |
| `cross-interface` | Interfaces |
| `operations` | Delivery And Operations |
| `proactive-analysis` | Assessment |

Secondary subsystem participation is expected. For example, observation views
consume state from every subsystem, and policy checks affect almost every
mutation. The primary owner is responsible for ensuring the behavior area has a
complete architecture story.

## Accepted System Decisions

- Tanren is self-hosted open-source infrastructure. Managed hosting is not a
  separate system architecture.
- Docker Compose is the baseline packaging and local/team operating profile,
  while equivalent container orchestrators may run the same service contracts.
- Postgres backs the canonical event log and projected read models.
- Every durable change is represented as a typed event.
- Repo-local docs, specs, generated commands, proof files, and roadmap views
  are projections of typed Tanren state unless explicitly classified otherwise.
- Tanren-owned projection drift is rejected and remediated by regeneration from
  canonical state.
- Responsive web UI is a first-class interface alongside CLI, TUI, API, and
  MCP.
- One Tanren project maps to exactly one source-control repository.
- Every Tanren project requires a mergeable source-control provider before it
  can execute specs.
- Solo and team modes use the same identity, policy, configuration, and audit
  data model.
- Agent execution uses containers or remote execution targets, not unmanaged
  local worktrees.
- Codex, Claude Code, and OpenCode are required harness adapter families.
- Provider integrations and client integrations are separate architectural
  concepts even when they involve the same external platform.
- Client integrations can use the same public capability surface as
  first-party clients where policy allows.
- Every event envelope includes event ID, event type, schema version,
  occurred-at timestamp, actor, scope, correlation ID, causation ID,
  idempotency key where applicable, source interface, redaction class, and
  visibility metadata.
- Required shared read models cover project overview, behavior proof status,
  roadmap progress, active specs, queues and execution targets, findings,
  integrations, notifications, configuration, audit, and observation reports.
- The Compose baseline uses separate named services for API, web, MCP, daemon,
  runtime worker, projection worker, provider worker, webhook worker,
  notification worker, Postgres, and one-shot migration/bootstrap jobs.
- First-run bootstrap uses a one-time installation token to create the first
  admin through normal API policy, then revokes that bootstrap path.
- TypeScript is the only first-party client SDK for v1. OpenAPI remains the
  cross-language contract for other clients.

## Rejected Alternatives

- **SQLite as the default local state architecture.** Rejected because local and
  team installs should share the same Postgres-backed event and projection
  model.
- **Repo files as primary source of truth.** Rejected because cross-interface,
  multi-user, replayable, auditable behavior requires typed event canon and
  generated projections.
- **Separate solo/local architecture.** Rejected because solo builders should be
  able to grow into team use without migration to a different product model.
- **Unmanaged local worktree execution as a core runtime path.** Rejected
  because container and remote execution targets provide clearer policy,
  secret, proof, and cleanup boundaries.
- **MCP as the general public API.** Rejected because MCP is the agent tool
  surface; public clients need API contracts with idempotency, versioning,
  subscriptions, and webhook semantics.
- **Observation as interface-local dashboards.** Rejected because status,
  reports, summaries, provenance, and bounds must be proof- and source-backed
  projections from canonical state.

## Architecture Record Suite

The system architecture is decomposed into records with explicit ownership.
Each record should state purpose, behavior coverage, accepted decisions,
component boundaries, data and contract model, key flows, failure and recovery
model, security and policy concerns, open questions, rejected alternatives, and
roadmap implications.

Required records:

- `technology.md` - implementation language, framework, database, queue,
  frontend, packaging, and validation choices.
- `delivery.md` - stack install, generated assets, repo bootstrap, container
  packaging, Compose baseline, upgrades, uninstall, and projection drift
  handling.
- `operations.md` - operating modes, backup/restore, disaster recovery,
  health, pause/resume/drain, schedules, cost/quota, incident and safe modes,
  and audit export.
- `subsystems/state.md` - event log, event envelope, projections, read models,
  subscriptions, idempotency, and replay.
- `subsystems/interfaces.md` - CLI, TUI, responsive web UI, API, MCP,
  validation errors, cross-interface continuity, and freshness.
- `subsystems/identity-policy.md` - accounts, organizations, permissions,
  roles, service accounts, API keys, approvals, and policy evaluation.
- `subsystems/configuration-secrets.md` - configuration tiers, effective
  config, inheritance, secrets, credentials, rotation, revocation, and usage.
- `subsystems/planning.md` - product, behavior catalog, architecture, roadmap,
  decisions, proposals, and assumptions.
- `subsystems/assessment.md` - implementation assessment, spec-independent
  analysis, findings, recommendations, intake classification, and routing.
- `subsystems/orchestration.md` - specs, tasks, phases, gates, findings,
  review, merge, cleanup, team coordination, and autonomy integration.
- `subsystems/quality-controls.md` - automated gates, audit, adherence,
  run-demo critique, task/spec guards, active-spec findings, and phase
  taxonomy.
- `subsystems/runtime.md` - workers, queues, leases, placement, execution
  targets, cancellation, retry, recovery, and worker-scoped access.
- `subsystems/provider-integrations.md` - source control, CI, issue trackers,
  infrastructure providers, identity providers, notification providers,
  external analysis providers, external action audit, and provider
  reconciliation.
- `subsystems/client-integrations.md` - external clients calling Tanren,
  webhooks, schema versions, idempotent requests, subscriptions, rate limits,
  backpressure, and replay.
- `subsystems/observation.md` - dashboards, metrics, reports, digests,
  summaries, provenance, trends, forecasts, bounds, and redaction-aware
  proof/source links.
- `subsystems/behavior-proof.md` - BDD behavior proof, behavior-to-feature
  linkage, positive and falsification witnesses, coverage interpretation, and
  mutation-testing proof-quality signals.
