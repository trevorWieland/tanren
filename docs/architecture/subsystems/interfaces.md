---
schema: tanren.subsystem_architecture.v0
subsystem: interfaces
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Interfaces Architecture

## Purpose

This document defines Tanren's own public interface architecture. It owns the
contract rules for Tanren's responsive web UI, HTTP API, MCP service, CLI, and
TUI so every surface exposes the same product state, command semantics, policy
behavior, idempotency behavior, projection freshness, and audit trail.

Project-general surface modeling lives in
[`experience-surfaces.md`](experience-surfaces.md) and
[`docs/experience/surfaces.yml`](../../experience/surfaces.yml). This document
is intentionally scoped to Tanren's current first-party surfaces.

Interfaces do not own Tanren truth. They authenticate actors, present
capabilities, submit commands, read projections, subscribe to updates, and
show behavior proof or source-backed status. Durable truth remains the event
log. Current state remains read models and projections.

## Subsystem Boundary

The interfaces subsystem owns:

- stable behavior interface IDs;
- public command/query/subscription contract rules;
- cross-interface error taxonomy;
- idempotency behavior exposed to clients;
- pagination, filtering, sorting, cursor, and freshness conventions;
- redaction and visibility behavior at the interface boundary;
- capability discovery and unsupported-action responses;
- event-stream access rules for clients;
- first-party client roles for web, CLI, TUI, API, and MCP;
- generated contract expectations for first-party clients.

The interfaces subsystem does not own identity policy, event append rules,
projection implementation, runtime execution, provider behavior, or product
workflow meaning. Those decisions belong to their owning subsystems.

## Core Invariants

1. **All public clients use the HTTP control plane.** Web, CLI, TUI, API
   clients, and MCP clients reach Tanren through network service contracts.
   There is no direct local application-service bypass for first-party clients.
2. **One command means one thing.** Equivalent operations across web, CLI, TUI,
   API, and MCP must reach the same application service and produce the same
   event semantics.
3. **The API is the general machine contract.** The web UI, CLI, TUI, external
   automation, event-stream consumers, and integrations consume API-backed
   commands, queries, subscriptions, and read models.
4. **MCP is the agent/tool contract.** MCP exposes agent-appropriate tools over
   HTTP, backed by the same application services and permission model as the
   API.
5. **Permissions shape capability, not interface forks.** The actor's identity,
   permissions, service-account scope, assignment context, and capability
   claims determine what a surface can do.
6. **Interfaces never treat projections as canon.** Clients may display repo
   artifacts, read models, and generated files, but mutations go through typed
   commands that append events.
7. **Freshness is visible.** Read models, subscriptions, and command results
   expose cursor or freshness metadata where stale projection state matters.
8. **Unsupported actions are explicit.** Interfaces must return stable
   unsupported-action or permission-denied responses instead of silently
   hiding behavior or falling back to local assumptions.

## Tanren Surface IDs

Tanren behavior files currently use stable interface IDs to state which
surfaces must support a behavior. During the migration to project-defined
surfaces, these IDs are also declared in `docs/experience/surfaces.yml`, and
validators treat behavior `interfaces:` as a compatibility alias for
`surfaces:`.

The accepted Tanren IDs are:

| ID | Description |
|----|-------------|
| `web` | Responsive web UI for the primary human product experience across desktop and phone. |
| `api` | HTTP API for first-party clients, external automation, event-stream access, and machine integrations. |
| `mcp` | MCP Streamable HTTP service for agents and LLM tool clients. |
| `cli` | Scriptable command-line client for power users, operators, installation, administration, and automation. |
| `tui` | Terminal UI client for power users and live operational workflows. |

There is no `any` interface marker. If a behavior applies to every public
surface, it lists every public surface explicitly. This forces behavior
coverage to name the actual contract obligations.

The daemon, scheduler, workers, and projection runners are internal actors, not
public interface IDs. Their behavior belongs to runtime, orchestration, state,
operations, or integration architecture records.

## Surface Roles

### Web

The web UI is the default human product surface. It supports planning, behavior
review, architecture review, implementation assessment, roadmap inspection,
spec and task workflows, observation, approvals, proof inspection,
configuration, integrations, account administration, recovery, behavior-proof
views, and operational status.

The web UI consumes the public API through generated TypeScript clients. It
does not use a separate backend-for-frontend layer, maintain duplicate API
shapes, or store server state as a divergent client-side source of truth.

The web UI must be responsive enough to support desktop and phone workflows
from the same application.

### API

The HTTP API is Tanren's general machine contract. It exposes:

- commands that append canonical events;
- queries over read models and projections;
- raw event-stream access for permitted clients;
- subscriptions and cursor recovery;
- service account and administrative operations;
- integration and webhook management;
- export, diagnostics, and recovery operations.

The API is consumed by first-party clients and external clients. It is
documented through generated OpenAPI from Rust contract types and route
metadata.

### MCP

The MCP service is Tanren's agent-facing tool interface. It uses MCP
Streamable HTTP and is served as part of the same self-hosted control plane.

MCP exposes tools appropriate for agent use, including tools used by internal
Tanren workers and tools used by a builder's own agent clients. It is not a
separate product model. Tool calls authenticate as a user, service account, or
worker-scoped actor and reach the same command/query services as API-backed
operations.

MCP tool visibility and execution are permission and capability aware. Internal
workers should receive narrowly scoped credentials for their assignment. End
users may create personal, project, or organization service accounts for their
own agent clients. The same identity and policy model decides what tools are
visible and what calls are allowed.

### CLI

The CLI is a scriptable and operator-friendly client for installation,
administration, project workflows, diagnostics, export, and automation. It
communicates with Tanren through the HTTP API.

The CLI must not mutate local projection files as canonical state or link
directly into local application services as a bypass. When a CLI command
changes Tanren state, it sends a typed API command that is authenticated,
authorized, idempotent where required, audited, and recorded as events.

### TUI

The TUI is a terminal client for live operational workflows. It communicates
with Tanren through the HTTP API and subscriptions.

The TUI is optimized for queue and worker status, runtime target health, live
operation, approvals, recovery, incidents, and focused control actions. It may
surface raw event or diagnostic views for operators, but its default views
should consume operational read models rather than requiring users to interpret
raw event streams.

## Common Contract Model

Every public interface operates over the same conceptual contract families:

- **commands** request durable changes and append events when accepted;
- **queries** read projections and read models;
- **subscriptions** stream read-model or event-position updates with cursors;
- **exports** produce explicit, permissioned snapshots or event ranges;
- **capability metadata** describes what the authenticated actor can see and
  do in the current scope;
- **diagnostics** expose health, freshness, drift, and recovery information.

Interface ergonomics may differ. A web workflow may be multi-step, a CLI
workflow may be a single command, an MCP workflow may be a tool call, and a TUI
workflow may be a focused interaction. The accepted command, policy decision,
event emission, and resulting state must remain the same.

## Authentication And Actor Context

Every interface request resolves an actor context before side effects or
protected reads occur.

Actor context includes:

- authenticated principal;
- actor kind: user, service account, worker, provider, or system actor;
- account, organization, project, and repository scope;
- active permissions and capability claims;
- assignment or runtime context where applicable;
- request ID, correlation ID, and optional idempotency key.

Anonymous access may exist only for explicitly public installation metadata or
bootstrap flows. All product state, event streams, commands, secrets metadata,
proof, source records, and operational views require authenticated access.

## Authorization And Capability Discovery

Authorization is permission-based. Roles are permission collections, not
hard-coded personas.

Interfaces should expose capability metadata so clients can render available
actions, agent tools, and disabled states without guessing. Capability metadata
must be computed from the same policy model used to authorize command
execution.

Capability discovery is advisory. Enforcement still happens at command/query
execution time.

MCP tool discovery is capability aware. An agent should only see tools that
its credential is allowed to use in the current scope. API clients may receive
unsupported-action or permission-denied responses even if a route exists.

## Error Taxonomy

All public interfaces use a shared machine-readable error taxonomy:

| Code | Meaning |
|------|---------|
| `auth_required` | The request requires authentication or the credential is missing/expired. |
| `permission_denied` | The actor is authenticated but lacks the required permission or capability. |
| `validation_failed` | The request shape or domain input is invalid. |
| `not_found` | The requested resource does not exist or is not visible to the actor. |
| `conflict` | The request conflicts with current state or lifecycle rules. |
| `idempotency_conflict` | The idempotency key was reused with a different request. |
| `stale_projection` | The requested action or read depends on a projection that is too stale. |
| `drift_detected` | A Tanren-owned projection has drifted and must be regenerated or re-read. |
| `rate_limited` | The actor, credential, route, or installation has exceeded a limit. |
| `unavailable` | A required Tanren service or dependency is temporarily unavailable. |
| `unsupported_action` | The surface, actor, scope, or resource does not support the requested action. |
| `provider_failure` | An external provider returned or caused a failure. |
| `execution_failure` | A worker, harness, runtime target, or assigned execution failed. |
| `internal_error` | Tanren encountered an unexpected server-side failure. |

Each error response includes a stable code, human-readable summary, request or
correlation ID, and structured details where safe. Errors must not leak secrets
or hidden cross-scope resource existence.

## Idempotency

Mutating interface operations support idempotency when retry could otherwise
create duplicate work, duplicate events, duplicate external effects, or
ambiguous client outcomes.

API clients pass explicit idempotency keys. Web, CLI, TUI, and MCP clients
must generate or forward keys for retryable mutations. Replaying the same key
with the same request returns the prior accepted result or stable result
reference. Reusing the same key with a different request returns
`idempotency_conflict`.

Interfaces should show enough result metadata for clients to recover from
timeouts without guessing whether a command succeeded.

## Read Models, Freshness, And Cursors

Query responses that come from projections include freshness metadata where
the information can lag canonical events.

Freshness metadata may include:

- source event position;
- projection name and checkpoint;
- generated-at timestamp;
- staleness status;
- cursor for resuming subscriptions or paging.

Commands that require up-to-date projection state may reject with
`stale_projection` instead of proceeding from stale data. Clients should then
retry after the projection catches up or request a stronger read path if one is
available for that operation.

## Pagination, Filtering, And Sorting

List APIs use stable pagination and sorting contracts. Cursor pagination is
preferred for event streams, logs, long histories, and changing operational
feeds. Offset pagination may be used only where stable ordering and bounded
datasets make it safe.

Filtering and sorting fields are part of the public contract. Unsupported
filters or sorts return `validation_failed`, not silent fallback behavior.

## Subscriptions And Realtime

Realtime updates are convenience delivery over canonical events and read
models. They are not canonical state.

Public subscriptions use API-backed Server-Sent Events by default. Subscription
messages include enough cursor or position metadata for clients to reconnect
and recover missed updates.

Web and TUI use subscriptions for live state. CLI may expose watch commands.
MCP tools may consume subscription-backed context only where the tool protocol
and assigned capability make that appropriate.

## Event Stream Access

The API exposes permissioned raw event-stream access for advanced clients,
exports, backups, migrations, reporting, and custom automation.

CLI may provide export and inspect commands over the API event stream. TUI may
provide operator diagnostic views over the same API. MCP may expose event
inspection tools when the actor's capabilities permit it.

Raw event access is not the easiest interface for most clients. Read-model
queries remain the preferred integration path for common workflows.

## Redaction And Visibility

All interfaces enforce event, read-model, proof/source, and secret visibility
at the boundary.

Redaction rules:

- secret values are never returned;
- hidden resources must not leak through errors, counts, or autocomplete;
- proof summaries, source records, and runtime output are filtered by scope and
  permission;
- event payloads are redacted according to event visibility metadata;
- UI surfaces clearly distinguish redacted data from missing data where doing
  so does not leak protected information.

## Proof And Source Links

When interface responses summarize work, claims, audits, findings, demos, or
release outcomes, they should include links or references to supporting
behavior proof or source signals that the actor is allowed to see.

Proof and source references should be stable resource identifiers, not
interface-local paths. Interfaces may render those references differently, but
they should resolve through API-backed behavior-proof, assessment, runtime,
integration, or delivery models.

## Contract Generation

Public API contracts are generated from Rust contract types and route metadata.
The generated OpenAPI document is the source for TypeScript web clients and
may also support external client generation.

MCP tool schemas are generated from or declared against the same command/query
contract layer. CLI and TUI command implementations call the API contract
rather than maintaining separate domain logic.

Contract generation failures are build failures. Hand-written duplicate API
shapes in first-party clients are rejected.

## Accepted Interface Decisions

- Public interface IDs are `web`, `api`, `mcp`, `cli`, and `tui`.
- The legacy `any` interface marker is removed.
- `daemon` is not a public interface ID.
- All first-party clients communicate through the HTTP control plane.
- The API is the general machine contract.
- There is no backend-for-frontend layer in the core architecture.
- MCP is an agent/tool interface over Streamable HTTP, backed by the same
  application services and identity model as the API.
- MCP tool visibility and execution are governed by permissions,
  service-account scope, capability claims, and assignment context.
- CLI and TUI are power-user clients over the API, not local state bypasses.
- All public interfaces share one error taxonomy.
- Capability discovery is supported but does not replace enforcement.
- Raw event-stream access is exposed through the API and can be surfaced by
  CLI, TUI, or MCP where permissions allow.
- Behavior files must use explicit public interface IDs and may not use `any`
  or `daemon`.
- Strong read-after-write guarantees apply to actor session state, permission
  changes, idempotency records, active assignment leases, and provider action
  enqueue results.
- Builder-owned personal agents receive planning, observation, assessment, and
  allowed project workflow tools by default. Internal worker assignments
  receive only tools required for the assigned phase and scope.
- Rate-limit and budget policy apply before v1 to mutations, MCP tool calls,
  runtime provisioning, provider actions, webhook subscriptions, report
  exports, and raw event-stream access.
- Stable subscription channels are read-model updates, active spec progress,
  runtime assignment progress, provider action status, webhook delivery status,
  notification feed, observation digest status, and audit events where
  permitted.

## Canonical Session, Error, OpenAPI, And Design-Token Decisions

These decisions land with R-0001 and apply to every subsequent feature
that touches authenticated requests, error mapping, schema generation,
or web styling. They cross-reference
[`profiles/rust-cargo/architecture/cookie-session.md`](../../../profiles/rust-cargo/architecture/cookie-session.md),
[`profiles/rust-cargo/architecture/openapi-generation.md`](../../../profiles/rust-cargo/architecture/openapi-generation.md),
and
[`profiles/react-ts-pnpm/architecture/styling-and-design-tokens.md`](../../../profiles/react-ts-pnpm/architecture/styling-and-design-tokens.md).

### Cookie session for `@web` and `@api`

Successful sign-up, sign-in, and accept-invitation responses to web and
API clients set a session cookie:

```
Set-Cookie: tanren_session=<id>; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=2592000
```

The cookie is managed by `tower-sessions` with a SQLx-backed
`SessionStore` over the `account_sessions` table (see
[state](state.md)). The web invitation flow uses a server-rendered
interstitial accept page (a Next.js page) so the cookie-setting POST is
same-origin and `SameSite=Strict` fires correctly. Sign-out is
`POST /sessions/revoke`, which clears the cookie and deletes the
session row.

### Bearer session for `@cli`, `@mcp`, and `@tui`

CLI, MCP, and TUI clients have no cookie jar to use. Their session
responses carry the `SessionToken` in the response body, and clients
attach it as a bearer token on subsequent requests. The same backing
session row is created — the difference is purely transport.

### `SessionEnvelope` discriminator

The contract type is:

```rust
enum SessionEnvelope {
    Cookie { account_id: AccountId, expires_at: DateTime<Utc> },
    Bearer { account_id: AccountId, expires_at: DateTime<Utc>, token: SessionToken },
}
```

API and web responses receive `Cookie`; CLI/MCP/TUI receive `Bearer`.
The discriminator is part of the contract because a generated client
must know which transport it is using to know whether to attach
credentials from a cookie jar or from request state.

### Error taxonomy extension

The shared error taxonomy (above) gains `validation_failed` for empty
or malformed request input — HTTP 400. This is distinct from
`invalid_credential` (HTTP 401, "credentials don't match a user"):
empty-input requests must NOT map to 401, because doing so leaks
"credentials shape valid" as separate from "credentials accepted." Any
empty-field input returns `validation_failed`.

### OpenAPI generation

OpenAPI 3.1 documents are generated from `utoipa` annotations on
handlers and `OpenApi` derives on the contract types. `utoipa-axum`
provides `OpenApiRouter` so the router and the schema stay in sync.
The generated document is served at `/openapi.json`.

Hand-rolled `serde_json::json!({...})` OpenAPI definitions are
forbidden. Contract drift is caught at compile time by the same
generation that produces the document, not at review time by a human
re-reading two files. OpenAPI examples never include real secret
values.

### CORS configuration

`tanren-api-app::Config::cors_allow_origins` is a `Vec<HeaderValue>`.
The dev default is `vec!["http://localhost:3000"]`. Production must set
`TANREN_API_CORS_ORIGINS` (comma-separated). `tower_http::cors::Any` is
denied in `tanren-api-app` via `clippy.toml` `disallowed_types` so a
future contributor cannot accidentally widen CORS to "any origin."

### Design tokens (web)

All colors and spacing flow through Tailwind v4's `@theme` block in
`apps/web/src/app/globals.css`. The token palette uses oklch values so
luminance-aware adjustments stay perceptually correct.

Inline `style={{ background: "#…" }}` for color or spacing is
forbidden; the oxlint rule `react/forbid-dom-props: ["style"]` (with a
narrow allowlist for CSS custom properties only) enforces this. The
component layer reads tokens through Tailwind utility classes, never
through arbitrary hex literals.

## Rejected Alternatives

- **`any` as a behavior interface marker.** Rejected because explicit interface
  lists create clearer behavior coverage obligations.
- **`daemon` as a public interface.** Rejected because daemon, scheduler,
  projection, and worker behavior belongs to internal runtime architecture.
- **Direct local service access for CLI and TUI.** Rejected because it would
  fragment authentication, authorization, event append, idempotency, and audit
  semantics.
- **Backend-for-frontend layer.** Rejected because the core architecture can
  serve the web UI through generated API clients and purpose-built read models.
- **MCP as an unrelated partial product model.** Rejected because agent tools
  must use the same command/query services, events, policy, and audit model.
- **Interface-local error models.** Rejected because automation, agents, and
  users need consistent failures across surfaces.
