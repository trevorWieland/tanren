---
schema: tanren.subsystem_architecture.v0
subsystem: client-integrations
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Client Integrations Architecture

## Purpose

This document defines Tanren's client integration architecture. Client
integrations are the inbound and subscription-based contracts external systems
use when they call Tanren, observe Tanren, automate Tanren, or receive Tanren
webhooks.

Client integrations let external automation act as a real Tanren client:
scripts, CI jobs, source-control automations, Zapier-style workflows,
organization automation, builder-owned agents, reporting systems, and custom
interfaces can create, update, observe, and synchronize Tanren state through
stable public contracts.

Client integrations do not own the shared API, MCP, CLI, TUI, or web contract
model. The interfaces subsystem owns public interface rules. Client
integrations own the integration-specific behaviors built on those contracts:
machine identity, attribution, schema negotiation, idempotency, subscriptions,
webhooks, replay, rate limits, backpressure, and external status reporting.

## Subsystem Boundary

The client integrations subsystem owns:

- integration client identity and attribution rules;
- service-account and API-key use from the client perspective;
- client access to public API and MCP capabilities;
- integration contract version negotiation;
- idempotent create and update request behavior;
- replay behavior after client, network, or Tanren failure;
- machine-readable integration errors;
- integration rate-limit, quota, and backpressure responses;
- read-model and subscription integration semantics;
- webhook endpoint configuration;
- webhook signing, delivery, retry, dedupe, and failure handling;
- client-reported source-control and CI status;
- integration audit records;
- client compatibility and deprecation metadata.

The client integrations subsystem does not own:

- provider integrations where Tanren calls external systems;
- source-control, CI, issue tracker, infrastructure, or notification provider
  adapter mechanics;
- the internal database or event schema;
- the general API framework, routing, or generated client mechanics;
- identity-policy evaluation;
- credential or secret value storage;
- projection implementation;
- orchestration lifecycle semantics;
- behavior-proof semantics;
- observation dashboards.

Provider integrations and client integrations can involve the same external
platform. A CI service may call Tanren through client integrations to report
status, while Tanren may also call that CI service through provider
integrations to poll status or request a rerun.

## Core Invariants

1. **External automation is a first-class client.** Tanren supports external
   systems, scripts, CI jobs, custom interfaces, and builder-owned agents using
   the same capability model as first-party clients where policy allows.
2. **No anonymous automation.** Every client request is attributed to a user,
   service account, API key, worker-scoped actor, provider actor, or other
   explicit machine identity.
3. **Policy applies before mutation.** Client integrations do not bypass
   account, organization, project, permission, approval, credential-use, or
   redaction policy.
4. **Automation writes are replay-safe.** Create and update requests intended
   for automation are idempotent, or they fail explicitly when replay safety
   cannot be guaranteed.
5. **Public contracts are versioned.** API, webhook, event-stream,
   read-model, and MCP payload contracts evolve through explicit public schema
   versions, independent of internal storage schemas.
6. **Read models are the default observation path.** Most clients observe
   current Tanren state through permissioned read models and subscriptions,
   not by scraping UI or depending on internal tables.
7. **Webhooks are at-least-once delivery.** Receivers must dedupe, tolerate
   retries, and tolerate reordering. Tanren exposes enough metadata for that.
8. **Webhook payloads are bounded and redacted.** Webhooks include useful
   payloads for common receivers, but large, sensitive, or hidden data is
   represented by resource references and cursors.
9. **External status reports are linked.** Client-reported source-control or
   CI status must reference known Tanren work, known source-control resources,
   or known CI resources.
10. **Backpressure is explicit.** Rate limits, quota limits, maintenance mode,
    queue pressure, and policy throttles return machine-readable retry or
    stop guidance.

## Client Types

Client integrations support several machine-client classes:

- **API clients** use Tanren's `/api/v1` HTTP contract for commands, queries,
  subscriptions, status reports, webhook management, and administration.
- **External MCP clients** use Tanren's MCP service when an agentic tool
  surface is the appropriate contract.
- **Service-account clients** act as non-human automation identities with
  explicit grants and audit visibility.
- **Webhook consumers** receive Tanren event deliveries through configured
  endpoint subscriptions.
- **CI and source-control reporters** submit external status when status is not
  being collected through provider integrations.
- **Custom interfaces** build alternative human or machine workflows on top of
  public Tanren contracts.
- **Organization automation** provisions projects, configuration, credentials,
  access, or operational policy where permitted.
- **Builder-owned agents** interact with Tanren through API or MCP using the
  same scoped capability system as internal workers.

Client type does not determine permissions by itself. Identity-policy decides
what each actor can do.

## Capability Surface

Client integrations may access the same public capability surface as
first-party clients when the actor has permission.

This includes:

- planning and intake commands;
- behavior and architecture planning commands;
- roadmap and proposal commands;
- spec and orchestration commands;
- review, walk, and merge-support commands;
- configuration and provider-management commands;
- read-model queries;
- event-stream access;
- webhook management;
- external status reporting;
- audit and history inspection.

Tanren should not create a second-class integration API that can only perform a
small subset of work. If a builder wants to build a custom UI, a workflow
automation, or an external agent that uses Tanren end to end, the architecture
must support that through policy, capabilities, contract versioning, and
redaction.

Unsupported actions return explicit capability or policy errors. They do not
fall back to hidden first-party-only paths.

## API Contract

The public REST API base path is `/api/v1`.

The API supports:

- authenticated commands;
- read-model queries;
- subscriptions;
- raw event-stream access where permitted;
- webhook endpoint management;
- idempotency keys;
- schema and compatibility discovery;
- machine-readable error responses;
- rate-limit and backpressure metadata;
- request attribution and audit identifiers.

The `/api/v1` path identifies the major API contract generation. Individual
payloads, webhook deliveries, event-stream messages, and long-lived public
schemas carry explicit schema version metadata when they need independent
compatibility handling.

Internal event types, database tables, Rust structs, and projection schemas are
not public API contracts unless explicitly exported as public schemas.

## MCP Contract

MCP is a public agent tool contract for Tanren-aware clients.

External MCP clients may include:

- a builder's personal agent session;
- a team-owned automation agent;
- a custom agent interface;
- an integration platform that exposes Tanren tools to another workflow.

MCP tools are filtered by the actor's permissions, scope, and capability
policy. A client should only see and invoke tools that are meaningful and
allowed in its current context.

MCP is not a bypass around the API's security model. MCP mutations emit the
same typed Tanren events, obey the same policy checks, use the same
idempotency model where applicable, and produce the same audit attribution as
API mutations.

## Identity, API Keys, And Attribution

Client integrations depend on identity-policy for actor identity, grants,
permission evaluation, service accounts, and API key lifecycle.

Client integration records include:

- client identifier;
- actor kind;
- owner scope;
- credential metadata;
- permission boundary;
- allowed interface contracts;
- last-used metadata;
- request source metadata where available;
- rate-limit and quota policy;
- audit visibility.

Every write or status report records:

- actor identity;
- credential or service-account class;
- owner scope;
- request source;
- affected Tanren scope;
- action category;
- request identifier;
- idempotency key where provided;
- resulting event or resource references.

Secret values, API key values, webhook signing material, bearer tokens, and
private credentials are never returned in attribution records.

## Idempotency

Automation-facing create and update requests require replay safety.

Idempotency records include:

- actor identity;
- scope;
- command kind;
- idempotency key;
- request fingerprint;
- first-seen timestamp;
- final or pending result reference;
- conflict status;
- expiration policy.

If the same actor repeats the same compatible request with the same
idempotency key, Tanren returns the same result or continues the pending
operation. If the actor reuses the key for a conflicting request, Tanren
rejects it with a machine-readable idempotency conflict.

Requests that cannot safely support idempotency must be explicit about that in
the contract. Replay-unsafe mutation is not allowed as an accidental default
for public automation.

## Replay And Recovery

Client integrations support replay after client timeouts, network failures,
server restarts, projection lag, and uncertain command outcomes.

Replay may return:

- the existing completed result;
- the current pending result;
- a conflict error;
- a stale projection error with retry guidance;
- a terminal failure and recovery hint.

Replay must not create duplicate visible work, duplicate provider side
effects, duplicate webhook endpoints, duplicate API keys, or conflicting
ownership records.

Operators can inspect replay attempts and outcomes where visible under policy.

## Machine-Readable Errors

Integration errors use the shared error taxonomy defined by interfaces, with
integration-specific metadata.

Common categories include:

- `validation_failed`;
- `permission_denied`;
- `policy_denied`;
- `not_found`;
- `redacted`;
- `unsupported_version`;
- `unsupported_capability`;
- `idempotency_conflict`;
- `stale_projection`;
- `rate_limited`;
- `quota_limited`;
- `backpressure`;
- `maintenance_mode`;
- `provider_failure`;
- `webhook_delivery_failed`;
- `replay_required`;
- `conflict`;
- `internal_error`.

Errors include a stable category, affected field or scope where safe, request
identifier, and remediation hint where possible. Errors must not reveal hidden
resources, secret values, unrelated client details, or private workloads.

## Versioning And Compatibility

Public integration contracts are versioned independently from internal
implementation schemas.

Versioned surfaces include:

- `/api/v1` route generation;
- request and response schemas;
- read-model schemas;
- event-stream message schemas;
- webhook payload schemas;
- MCP tool schemas;
- error schemas;
- status-report schemas.

Clients can discover supported versions for the relevant contract. Unsupported
versions fail with a machine-readable compatibility error. Deprecation and
migration guidance should be visible before a public contract is removed where
possible.

Tanren is not required to support every historical contract forever. During
pre-1.0 development, breaking changes are acceptable, but public contract
versioning remains part of the architecture so v1 clients have a stable model.

## Observation Contracts

Integration clients observe Tanren through:

- read-model queries;
- cursor-based pagination;
- subscriptions;
- raw event-stream access where permitted;
- webhook deliveries;
- export APIs where permitted.

Read models are preferred for most integrations because they provide stable,
purpose-built views with redaction and freshness metadata. Raw event access is
for advanced clients such as reporting, backup, migration, custom automation,
or deeply integrated external systems.

Observation responses include:

- stable resource identities;
- schema version;
- freshness or projection checkpoint metadata;
- cursor metadata for paging or resuming;
- redaction markers where safe;
- links to supporting behavior proof or source signals where visible.

Clients must not depend on UI HTML, internal database shapes, private Rust
types, or repo-local projection file layouts as machine contracts.

## Subscriptions

Subscriptions provide live or near-live updates over public contracts.

Subscription records include:

- subscription identifier;
- actor identity;
- scope;
- selected resource types or event categories;
- cursor position;
- delivery transport;
- redaction policy;
- status and health metadata.

Server-Sent Events are the default realtime subscription transport for API
clients. Subscription messages include cursor or position metadata so clients
can reconnect and recover missed updates.

Subscriptions are convenience delivery over canonical events and read models.
They are not an independent source of truth.

## Webhook Endpoints

Webhook endpoints are configured client integrations that receive Tanren
events through scoped subscriptions.

Endpoint records include:

- endpoint identifier;
- owner scope;
- destination URL;
- event categories or resource filters;
- signing configuration metadata;
- schema version;
- payload mode;
- delivery policy;
- retry policy;
- health state;
- last delivery summary;
- pause, resume, disable, and removal state.

Webhook signing material is secret. It is shown only at creation or rotation
time when applicable and is otherwise represented by non-secret metadata.

Webhook endpoints can be created, updated, paused, resumed, disabled, removed,
and retried by actors with permission.

## Webhook Payloads

Webhooks deliver bounded, redacted payloads by default.

Webhook payloads include:

- delivery identifier;
- event or message identifier;
- event category;
- schema version;
- timestamp;
- source event position or cursor where available;
- resource identities;
- compact redacted resource snapshot where safe;
- changed fields or summary where safe;
- dedupe metadata;
- attempt metadata;
- links for permitted follow-up reads.

Large payloads, sensitive fields, hidden resources, raw runtime output, secret
metadata, and policy-sensitive details are omitted or replaced with references.
Receivers can fetch additional detail through read models if they have
permission.

Webhook payload size limits are part of endpoint policy. When an event cannot
fit safely in the bounded payload, Tanren delivers a compact reference payload
rather than failing or leaking oversized data.

## Webhook Delivery

Webhook delivery is at-least-once.

Delivery semantics:

- duplicate deliveries are possible;
- receivers must dedupe using stable event and delivery metadata;
- Tanren includes global cursor metadata where available;
- per-endpoint delivery order is best effort;
- receivers must tolerate reordering;
- delivery retries follow visible policy;
- failed deliveries remain visible and attributable;
- hidden events are never delivered outside the endpoint's configured scope.

Webhook delivery states include:

- `pending`;
- `delivered`;
- `failed_retryable`;
- `failed_terminal`;
- `paused`;
- `disabled`;
- `skipped`;
- `expired`.

Webhook failure handling supports retry, pause, resume, disable, and endpoint
repair where policy allows.

## External Status Reporting

Client integrations allow permitted clients to report external CI or
source-control status when that status is not collected directly through a
provider integration, or when a provider uses Tanren's public client contract
to report it.

External status reports must reference at least one known resource:

- Tanren spec;
- Tanren graph node;
- Tanren task or active work item;
- source-control repository, branch, commit, or pull request known to Tanren;
- CI provider resource known to Tanren;
- provider integration resource mapping known to Tanren.

Tanren does not accept arbitrary opaque status contexts that cannot be tied to
known Tanren work or known source-control/CI resources.

Status categories include:

- `pending`;
- `passing`;
- `failing`;
- `cancelled`;
- `skipped`;
- `unavailable`;
- `stale`;
- `conflicting`.

Client-reported status and provider-polled status normalize into common read
models while preserving origin, actor, provider, timestamp, schema version, and
source reference.

External status is not user acceptance, behavior proof, or merge approval by
itself. Orchestration and assessment decide how status affects active work or
post-hoc findings.

## Rate Limits And Backpressure

Client integrations expose machine-readable pressure signals.

Pressure sources include:

- per-client rate limits;
- per-scope quotas;
- queue pressure;
- projection lag;
- maintenance mode;
- incident mode;
- provider pressure surfaced through Tanren;
- budget or cost policy;
- abuse or safety throttles.

Responses identify whether the boundary is rate limit, quota, queue pressure,
maintenance, or policy. Retry guidance is machine-readable when retry is
allowed.

Operators can inspect aggregate pressure and client impact where visible under
policy. Client pressure views must not expose other clients' secrets, private
workloads, hidden resources, or unrelated operational details.

## Security And Policy

Security requirements:

- every client request is authenticated unless explicitly public and read-only;
- mutation requests require permission checks before events are appended;
- service-account and API-key ownership is explicit;
- client credentials are scoped, expirable, rotatable, and revocable;
- public errors do not leak hidden resources;
- all secret values and signing material are redacted;
- webhook payloads obey scope and redaction policy;
- raw event-stream access is permissioned and redacted;
- rate limits and quotas apply by actor, scope, and installation policy;
- integration audit records are visible only where policy allows.

Client integrations should make security visible and debuggable. Permission
denials identify safe categories of missing permission, scope, or policy
constraint without revealing protected details.

## Events

Client integrations emit typed events for:

- integration client created, updated, disabled, or removed;
- client credential created, rotated, revoked, expired, or used;
- client schema version negotiated or rejected;
- client command accepted, replayed, conflicted, or rejected;
- idempotency record created, completed, expired, or conflicted;
- subscription created, updated, paused, resumed, disabled, or removed;
- webhook endpoint created, updated, paused, resumed, disabled, or removed;
- webhook delivery queued, delivered, failed, retried, skipped, or expired;
- webhook signing material rotated;
- external status report accepted, rejected, superseded, or marked stale;
- rate-limit or backpressure boundary reached;
- integration audit record appended.

Events may record credential identifiers and non-secret metadata. Events must
not record API key values, webhook signing secrets, bearer tokens, or hidden
payload details.

## Read Models

Required client integration read models include:

- integration client list and detail;
- service-account and API-key usage summaries;
- integration audit history;
- idempotency and replay state;
- public contract version support;
- read-model and subscription catalog;
- subscription state and cursor health;
- webhook endpoint list and detail;
- webhook delivery history and failure queue;
- client-reported external status by spec, commit, PR, graph node, and source;
- rate-limit, quota, and backpressure summaries;
- client compatibility and deprecation warnings.

Read models must include freshness and redaction metadata where applicable.

## Accepted Decisions

- Client integrations and provider integrations are separate architecture
  subsystems.
- Client integrations use the same capability model as first-party clients
  where policy allows.
- `/api/v1` is the public REST API base path.
- Public payloads, webhooks, event-stream messages, and MCP tools can carry
  explicit schema version metadata.
- MCP is a valid public agent integration contract, not only an internal worker
  mechanism.
- Automation-facing writes are idempotent or explicitly replay-unsafe and
  rejected for unsafe automation use.
- Webhooks use bounded redacted payloads with references for large, sensitive,
  or hidden details.
- Webhook delivery is at-least-once with dedupe metadata.
- Webhook ordering uses global cursor metadata and best-effort endpoint order.
- External status reports must link to known Tanren work or known
  source-control/CI resources.
- Client-reported and provider-polled external status normalize into common
  read models while preserving origin.
- TypeScript is the first-party generated SDK for v1. OpenAPI remains the
  stable cross-language contract for other clients.
- Webhook payloads default to a 256 KiB body limit and use references for
  larger, sensitive, or hidden details.
- Stable webhook categories cover project, behavior, roadmap, spec,
  orchestration, proof, assessment, provider, webhook, notification, and
  operations summaries that are safe for the subscriber's grants.
- Mutating client commands that create durable work, provider side effects,
  webhooks, runtime assignments, approvals, or imports require explicit
  idempotency keys.
- Successful webhook delivery bodies are retained for 30 days. Failed delivery
  diagnostics are retained for 90 days.
- Before v1, compatibility is best-effort with explicit schema versions. At v1,
  `/api/v1`, webhook schemas, and MCP public tool contracts follow versioned
  compatibility policy.
- Compatibility fixtures cover GitHub webhook/status ingestion, generic CI
  status reporting, Slack-compatible outbound webhook delivery, and a minimal
  custom API client.

## Rejected Alternatives

- **Provider and client integrations as one subsystem.** Rejected because
  outbound provider mechanics and inbound client contracts have different
  security, retry, schema, and ownership concerns.
- **Integration clients as second-class clients.** Rejected because builders
  need custom interfaces, external agents, and automation to use Tanren
  end-to-end through public contracts.
- **Anonymous automation.** Rejected because auditability, permission
  boundaries, replay, and recovery require attributed client identity.
- **Exactly-once webhook delivery.** Rejected because external receivers and
  networks cannot support that guarantee reliably. At-least-once delivery with
  dedupe metadata is the correct contract.
- **Webhook references only.** Rejected because common receivers should be able
  to handle routine events without a follow-up fetch.
- **Full unbounded webhook payloads.** Rejected because payload size,
  redaction, hidden resources, and sensitive details require bounded delivery.
- **Opaque external status contexts.** Rejected because status that cannot link
  to known Tanren work or known source-control/CI resources creates ambiguous
  behavior and weakens traceability.
- **Public contracts mirroring internal schemas.** Rejected because internal
  events, projections, database tables, and Rust structs must evolve without
  becoming accidental external APIs.
