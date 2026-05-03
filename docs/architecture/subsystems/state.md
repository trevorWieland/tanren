---
schema: tanren.subsystem_architecture.v0
subsystem: state
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# State Architecture

## Purpose

This document defines Tanren's durable state architecture. It is the authority
for how Tanren records change, exposes current state, replays history,
regenerates projections, detects drift, and supports integrations that build on
top of Tanren.

State architecture exists to preserve one rule: durable Tanren truth is an
append-only stream of typed events. Everything else is a projection,
read model, derived work record, or external artifact.

## Subsystem Boundary

The state subsystem owns:

- canonical event append rules;
- event envelope and ordering semantics;
- event visibility and redaction metadata;
- idempotency records for mutation commands;
- projection checkpoints and replay behavior;
- relational read-model derivation;
- repo-artifact projection ownership and drift handling;
- public event-stream access rules;
- subscription cursors and replay recovery;
- schema versioning and event evolution;
- deletion, tombstone, revocation, and redaction semantics.

The state subsystem does not own product semantics, policy rules, runtime
placement decisions, provider behavior, or interface-specific presentation.
Those subsystems decide what a command means. State decides how accepted
changes are durably recorded and replayed.

## Core Invariants

1. **Events are canonical.** Every durable state transition is represented by a
   typed event in the append-only event log.
2. **The global log is the source of ordering.** Tanren has one canonical
   ordered event log. Account, organization, project, resource, and aggregate
   scopes are indexed metadata over that log, not independent sources of truth.
3. **Read models are disposable.** Any relational current-state table,
   dashboard, report, repo file, queue view, or subscription view must be
   rebuildable from events plus non-canonical external inputs that are
   explicitly classified.
4. **Mutation is async-first.** Command handlers should append events quickly
   and let projections, queues, notifications, webhooks, and subscriptions
   catch up asynchronously unless a synchronous write is required for
   correctness.
5. **Projection freshness is explicit.** Interfaces must be able to see whether
   a read model is current enough for the action being attempted.
6. **Tanren-owned artifacts are controlled by Tanren.** Manual edits to
   generated artifacts are drift. Tanren may recreate controlled artifacts from
   canonical events without human approval.
7. **Visibility is part of state.** Event-stream access is supported, but event
   reads are filtered through permission, scope, and redaction metadata.
8. **Secrets are never canonical payloads.** Secret values do not enter events,
   projections, logs, proof records, or repo artifacts. Events may record secret
   metadata, references, rotation, revocation, and usage.

## Event Log

Tanren stores a single global append-only event log in Postgres. The log is the
canonical mutation history for planning, behavior, architecture, roadmap,
specs, tasks, orchestration, runtime, policy, configuration, integrations,
behavior proof, observation, review, release learning, and operational state.

Every event has both:

- an `event_id`, using UUIDv7 for durable identity and time-sortable
  correlation;
- a global `position`, assigned by the database and used for total ordering,
  replay, cursors, checkpoints, and subscriptions.

UUIDv7 is useful for identity and locality. The global position is the
authority for replay order.

Events are immutable after append. Corrections, reversals, redactions,
revocations, drift reports, and repairs are represented by later events.

## Event Envelope

Each event uses a common envelope so every subsystem can share replay,
visibility, auditing, idempotency, and projection infrastructure.

Required envelope fields:

- `event_id`: durable UUIDv7 event identity;
- `position`: global event-log position;
- `event_type`: stable namespaced event type;
- `schema_version`: event payload schema version;
- `occurred_at`: service-side event append timestamp;
- `actor`: user, service account, worker, provider, or system actor that caused
  the change;
- `scope`: account, organization, project, repository, or installation scope;
- `resource`: primary resource identity affected by the event;
- `correlation_id`: operation, workflow, request, or trace that groups related
  events;
- `causation_id`: command, event, job, webhook, or provider message that caused
  this event;
- `idempotency_key`: command idempotency key when the event came from an
  idempotent mutation;
- `visibility`: read restrictions, redaction class, and allowed audience
  metadata;
- `payload`: typed event-specific data.

Optional envelope fields may include provider references, external object IDs,
proof links, policy decision references, projection repair references, and
runtime placement metadata.

## Event Categories

Tanren event types are grouped by subsystem so command ownership and projection
ownership stay clear:

- planning events for product, behavior, architecture, implementation
  assessment, roadmap, assumptions, and decisions;
- orchestration events for specs, tasks, phases, gates, audits, walks,
  findings, reviews, and cleanup;
- runtime events for assignments, queues, leases, workers, execution targets,
  retries, cancellations, and recovery;
- identity and policy events for accounts, organizations, memberships, roles,
  permissions, approvals, service accounts, API keys, and policy decisions;
- configuration and secret events for configuration changes, credential
  metadata, rotation, revocation, and usage;
- integration events for provider connections, webhooks, notifications,
  external references, and delivery outcomes;
- behavior-proof events for proof targets, proof runs, positive witnesses,
  falsification witnesses, proof staleness, and proof-quality signals;
- observation events for snapshots, summaries, forecasts, provenance signals,
  alerts, digests, and operational status;
- projection events for drift detection, projection rebuilds, projection
  failures, repair starts, and repair completions.

Subsystem architecture records own their specific event vocabularies. This
document owns the envelope and append semantics those vocabularies must obey.

## Command To Event Flow

All public mutation surfaces follow the same state path:

1. A command enters through API, MCP, CLI, TUI, web UI, provider callback, or
   internal scheduled work.
2. The application service authenticates the actor and resolves scope.
3. Policy, configuration, validation, and idempotency checks run.
4. The service appends one or more typed events in a Postgres transaction.
5. The command returns an accepted result, command result, event cursor, or
   validation/policy error.
6. Projection, queue, outbox, webhook, notification, and subscription workers
   consume the new event position.

The preferred path is maximum asynchronous work after append. This keeps API
and MCP mutation throughput high and prevents interface latency from depending
on expensive derived work.

Synchronous writes are allowed only where correctness requires them, including:

- event append integrity;
- idempotency acceptance and replay of prior command results;
- command results that must be returned immediately;
- locks, leases, or queue claims that must exist before a worker can proceed;
- minimal read-model updates required to prevent invalid duplicate actions;
- projection checkpoints when a consumer processes events transactionally.

Every synchronous exception should be explicit in the owning subsystem record.

## Idempotency

Mutation commands that can be retried by clients, workers, providers, or
network infrastructure must support idempotency.

Idempotency records are keyed by actor, scope, command kind, idempotency key,
and request hash. A repeated command with the same key and request hash returns
the same accepted result or stable result reference. A repeated command with the
same key and a different request hash is rejected as an idempotency conflict.

Idempotency records are not a substitute for event canon. They are command
deduplication records that point at canonical events or command results.

## Projections And Read Models

Projections consume the global event log and maintain derived state for
queries, interface rendering, reports, subscriptions, queues, and operational
inspection.

Projection rules:

- every projection has an owner and a declared input event set;
- every projection records its last processed global position;
- projection lag and failure state are queryable;
- projections are rebuildable from events;
- projection rebuilds emit projection lifecycle events;
- projection code must tolerate replay and duplicate delivery;
- read models must not accept direct mutations that bypass events.

Tanren may use synchronous critical projections sparingly. Most projections are
asynchronous and expose cursor or freshness metadata so clients can decide
whether a read model is current enough.

## Repo Artifact Projections

Repo-local documents and generated files are projections when Tanren owns them.
This includes product docs, behavior docs, architecture docs, roadmap views,
spec files, task files, proof indexes, generated command files, standards
profiles, and machine-readable planning artifacts unless a file is explicitly
classified as user-owned input.

Tanren-owned artifact rules:

- generated artifacts include ownership metadata where the file format allows;
- artifact content is derived from canonical events;
- artifact hashes or equivalent fingerprints are tracked;
- manual edits are projection drift;
- drift is reported through events and observation read models;
- drifted artifacts are recreated from canonical events;
- dependent actions must not silently continue from edited projection content.

Human-authored source material can still enter Tanren. It must do so through
import, review, or edit commands that validate the input and emit typed events.

## Drift Handling

Projection drift is a state event, not an interface warning alone.

When Tanren detects drift in a controlled projection, it records:

- the artifact identity;
- the expected projection fingerprint;
- the observed fingerprint;
- the detecting actor or system process;
- the event position or projection version used for comparison;
- the remediation action taken.

The normal remediation is regeneration from canonical events. Because
Tanren-owned artifacts are controlled by Tanren, regeneration does not require
human approval. Protection comes from clear ownership classification, audit
history, and the ability to replay the canonical event stream.

## Public Event Stream Access

Tanren exposes event-stream access as a first-class API capability for advanced
clients, integrations, reporting, export, backup, migration, and custom
automation.

Public event-stream access rules:

- event reads are scoped by installation, account, organization, project, and
  resource permissions;
- event payloads are redacted according to visibility metadata;
- clients page or subscribe by global position cursor;
- event-stream responses include schema version and event type;
- clients should be able to resume from the last observed position;
- API documentation must distinguish raw event access from easier read-model
  queries.

Most clients should consume read models, command results, and subscription
feeds instead of rebuilding Tanren state from raw events. Raw event access is
available because Tanren is self-hosted infrastructure and should be buildable
on top of.

## Subscriptions

Realtime delivery is a derived convenience over canonical events and read
models. It is not itself canonical state.

Subscriptions expose cursor metadata based on global event position or
projection position. Clients recover from disconnects by resuming from a cursor
or by re-querying a read model. Missed realtime messages must never require
manual repair.

The default public subscription transport is Server-Sent Events through the
API. Internal workers may use durable queue tables, projection checkpoints,
outbox tables, and Postgres wakeup signals. Postgres `LISTEN`/`NOTIFY` may
wake consumers but does not replace durable event, queue, outbox, or checkpoint
state.

## Schema Evolution

Events include explicit schema versions. Historical events are immutable after
append.

For v1 and later:

- new event versions are added deliberately;
- readers decode by event type and schema version;
- compatibility code or upcasters translate older payloads into current domain
  shapes where needed;
- event rewrites are not used for ordinary evolution;
- projection rebuilds must be able to process all retained historical events.

Before v1, Tanren may make breaking event-schema changes and reset development
state when doing so keeps the product architecture cleaner. Pre-v1 state reset
is a development posture, not a v1 operational capability.

## Deletion, Redaction, And Secrets

The default deletion model is event-sourced:

- deleted resources receive tombstone events;
- revoked capabilities receive revocation events;
- corrected or hidden material receives redaction events;
- audit views reflect the deletion or redaction according to permissions.

Physical deletion is reserved for non-canonical sensitive material, expired
external blobs, local caches, or data that policy requires Tanren to purge.
Physical deletion must not be used to silently erase ordinary canonical
history.

Secret values never enter canonical event payloads. Secret-related events may
record metadata such as secret identity, owner, provider, scope, rotation time,
revocation time, usage classification, and policy status. Secret values live in
the configured encrypted-at-rest secret substrate and are referenced by opaque
identifiers.

## Audit Semantics

The event log is the audit canon. Audit tables, audit screens, reports, and
exports are projections over the event log with permission filtering and
redaction.

Audit projections should preserve:

- actor identity;
- command intent;
- scope and resource;
- policy decision context;
- causation and correlation;
- visible event result;
- proof or source links where permitted;
- projection or external delivery outcome where relevant.

Audit views must not depend on interface-local logs or raw runtime output as
their source of truth.

## Operational Recovery

State recovery is replay-oriented.

Supported recovery actions include:

- rebuilding all projections from the global event log;
- rebuilding one projection from a known position;
- recreating Tanren-owned repo artifacts;
- retrying outbox deliveries from durable delivery records;
- resuming subscriptions from cursors;
- reconciling workers, queues, and leases from event and queue state;
- exporting the event stream for backup or migration.

Operational tools should prefer replay and reconciliation over manual database
patching. Manual patching is an exceptional break-glass operation and should be
followed by explicit repair events or administrative records where possible.

## Accepted State Decisions

- Postgres is the durable state substrate.
- The global append-only event log is the canonical source of truth.
- Every durable change is represented as a typed event.
- Events have both UUIDv7 identities and database-assigned global positions.
- Account, organization, project, resource, and aggregate scopes are metadata
  over the global log.
- Mutation handling is async-first, with synchronous writes allowed only for
  correctness-critical records.
- Read models and repo artifacts are rebuildable projections.
- Tanren-owned artifact drift is remediated by regeneration from events.
- The public API exposes permissioned event-stream access for advanced clients.
- Normal clients are encouraged to use read models and subscription feeds.
- Pre-v1 may reset development event history during breaking schema changes.
- v1 and later require immutable historical events and version-aware decoding.
- Tombstone, revocation, and redaction events are the default deletion model.
- Secret values never enter events, projections, logs, proof records, or repo
  artifacts.
- Product, behavior, planning, architecture, roadmap, orchestration, proof,
  assessment, identity, policy, configuration, secrets metadata, provider
  integration, client integration, delivery, and operations events have
  first-class export/import guarantees.
- Synchronous projections are allowed only for idempotency records, assignment
  leases, uniqueness constraints, permission-critical visibility records, and
  provider action enqueue state where stale reads could create duplicate work.
- Default retention is 90 days for runtime diagnostic output, 30 days for
  successful webhook payload bodies, 90 days for failed webhook diagnostics,
  and policy-controlled retention for source payloads and non-canonical logs.
- External service accounts subscribe by default only to read-model update
  streams and webhook-safe event categories allowed by their grants. Raw event
  subscriptions require explicit high-permission grants.

## Canonical Store Trait, Sessions Table, And Clock Injection

These decisions land with R-0001 and apply to every subsequent store
adapter and store-consuming handler. They cross-reference
[`profiles/rust-cargo/architecture/trait-based-abstraction.md`](../../../profiles/rust-cargo/architecture/trait-based-abstraction.md)
and
[`profiles/rust-cargo/architecture/crate-layering.md`](../../../profiles/rust-cargo/architecture/crate-layering.md).

### `AccountStore` trait — port and adapter

The store crate (`tanren-store`) defines a single
`pub trait AccountStore: Send + Sync + std::fmt::Debug` covering all
account/membership/session/event/invitation reads and writes for the
account lifecycle. `SeaOrmStore` is the SQLite/Postgres adapter.
Application services (`tanren-app-services`) depend on
`&dyn AccountStore`, never on the concrete `SeaOrmStore`. The
`clippy.toml` for `tanren-app-services` denies
`tanren_store::SeaOrmStore` so handlers cannot regress to the concrete
type.

The trait is intentionally one trait, not split per aggregate. Sign-up
writes account + session + event in one logical unit; accept-invitation
writes account + membership + invitation update + session + two events.
Splitting the port forces handlers to take a fistful of trait objects
per call without making the boundary cleaner.

### Atomic invitation consume

Invitation acceptance is a single round-trip:

```sql
UPDATE invitations SET consumed_at = $1
  WHERE token = $2 AND consumed_at IS NULL
  RETURNING ...
```

`rows_affected = 0` paired with the row's existence determines whether
the invitation was already consumed, expired, or never existed. The
find→check→update sequence is forbidden because it admits a race.

A partial-unique constraint
`UNIQUE (token) WHERE consumed_at IS NULL` on the `invitations` table
provides belt-and-braces protection at the SQL layer.

### `account_sessions` table

The `account_sessions` table has an `expires_at TIMESTAMPTZ NOT NULL`
column. The default value at insert time is `now + 30 days`; the
verifier path filters `expires_at > now` so expired sessions cannot
authenticate. Schema-snapshot tests guard the column.

`tower-sessions-sqlx-store` adapts the table for HTTP cookie sessions
(see [interfaces](interfaces.md)). The session row stores the opaque
`SessionToken`; no secret value other than the token itself is ever
written to the table.

### Clock injection

Every store write that previously called `chrono::Utc::now()` now takes
`now: DateTime<Utc>` as a parameter. The store does not read the clock;
handlers thread `clock.now()` (a `&dyn Clock`) into store calls. The
`clippy.toml` for `tanren-store` denies `chrono::Utc::now`
(`disallowed_methods`) so the rule is mechanically enforced.

This makes time controllable in BDD scenarios (the harness installs a
deterministic clock) and removes a class of flake from store-level
tests.

### `test-hooks` feature gate

Fixture-seeding methods (`seed_invitation`, `seed_account_for_test`, …)
are gated on `#[cfg(feature = "test-hooks")]`. `tanren-store/Cargo.toml`
declares the feature; `tanren-testkit/Cargo.toml` enables
`tanren-store/test-hooks`; production binaries do not.

CI runs `cargo check --workspace` twice: once **without** `test-hooks`
(catches accidental production reliance) and once **with** (catches
lint failures in the gated path). The matrix is wired into `just ci`.

`xtask check-test-hooks` rejects any `pub fn` whose doc-comment matches
`(?i)test|fixture|seed` unless gated on
`cfg(any(test, feature = "test-hooks"))`.

## Rejected Alternatives

- **Relational tables as independent truth.** Rejected because direct table
  mutation would break replay, audit, projection regeneration, and consistent
  cross-interface behavior.
- **Per-project event logs as canon.** Rejected because global ordering is
  needed for audit, cross-project dependencies, operational replay, and
  installation-level subscriptions.
- **UUIDv7-only ordering.** Rejected because UUIDv7 helps locality and identity
  but does not replace an authoritative database position for replay.
- **Manual projection repair.** Rejected because Tanren-controlled artifacts
  should be recreated from canonical state, not patched into consistency.
- **Hidden internal-only event stream.** Rejected because Tanren is
  self-hosted infrastructure and should support advanced clients, export,
  migration, reporting, and custom automation.
- **Secret values in event payloads.** Rejected because event logs are durable,
  replayable, exportable, and widely projected.
