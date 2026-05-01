---
schema: tanren.technology_architecture.v0
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Technology Architecture

## Purpose

This document defines Tanren's concrete technology posture. It turns the system
architecture into implementation constraints for language, backend services,
frontend, API contracts, MCP transport, persistence, queues, containers,
observability, security, and validation.

Technology choices optimize for:

- strict typing across service and interface boundaries;
- replayable event-sourced state;
- self-hosted container deployment;
- simple operational dependencies;
- strong local and team development parity;
- agent execution isolation;
- behavior-proof-driven validation.

## Language And Workspace

Tanren is a Rust-first system. Rust is the implementation language for the
control-plane services, domain model, event contracts, projection services, API
server, MCP server, daemon/scheduler, worker runtime, CLI, TUI, harness
adapters, provider adapters, store, BDD proof crate, and support libraries.

Rust is chosen because Tanren needs compile-time enforcement of domain
boundaries, explicit error handling, memory safety, concurrency safety, and
low-overhead service binaries that run well in self-hosted containers.

Workspace layout:

- `bin/` contains runnable Rust binaries.
- `crates/` contains reusable Rust libraries organized by domain, contract,
  store, runtime, harness, policy, scheduler, orchestrator, observability, and
  app-service boundaries.
- `xtask/` contains repository maintenance and proof-support commands.
- `commands/` contains command source assets rendered into installed agent
  targets.
- `tests/bdd/` contains executable behavior proof scenarios.

Public Rust APIs use explicit types and domain newtypes where appropriate.
Library crates use `thiserror`; binaries may use `anyhow`. Production code
avoids `unsafe`, `unwrap`, `panic!`, `todo!`, `unimplemented!`, `println!`,
`eprintln!`, and `dbg!`.

Crate dependency rules (mechanically enforced by `xtask check-deps`):

1. `tanren-domain` does not depend on any other workspace crate. It is the
   leaf canonical-entity layer.
2. Interface binaries (`tanren-api`, `tanren-cli`, `tanren-mcp`, `tanren-tui`,
   `tanrend`) depend on `tanren-app-services` and `tanren-contract` for
   product behavior; they do not reach into store, runtime, or harness crates
   directly.
3. Only `tanren-store` owns SQL and database row details. Other crates work
   in domain types and command/event contracts.
4. Runtime and harness crates do not own policy decisions. Policy decisions
   come from `tanren-policy` as typed results.
5. `tanren-policy` returns typed decisions, not transport errors. Interface
   binaries translate denial reasons into transport-appropriate responses.
6. Contract crates (`tanren-contract`) are serialization/schema surfaces, not
   orchestration logic. Orchestration consumes contracts; contracts do not
   import orchestration.
7. Observability is structured and correlation-friendly. Crates emit tracing
   spans and events through `tanren-observability` rather than bespoke logging
   or printlns.

## Backend Runtime

Tanren backend services run on Tokio.

Tokio is the async runtime for:

- HTTP API service;
- MCP Streamable HTTP service;
- daemon and scheduler work;
- worker coordination;
- provider integration calls;
- notification and webhook delivery;
- projection and subscription processing.

Backend services should be structured as small binaries over shared application
services and contract crates. Service boundaries may run as separate containers
or as combined binaries where deployment simplicity requires it, but they must
not duplicate domain logic.

## HTTP API

The primary HTTP framework is Axum.

Axum is selected because it composes with Tokio, Hyper, and Tower; supports
typed extractors and responses; uses Tower middleware for authentication,
policy, tracing, timeouts, and request limits; and fits Tanren's need for a
strict shared contract layer across web UI, external clients, and internal
services.

API technology rules:

- Public API request and response bodies are Rust contract types.
- API routes use Axum handlers over application-service commands and queries.
- Middleware uses Tower layers for auth, policy context, tracing, timeouts,
  rate limits, request IDs, and body limits.
- Errors use stable machine-readable categories shared with MCP and CLI where
  the underlying operation is equivalent.
- Mutations support idempotency where retry would otherwise create duplicate
  visible work or external side effects.
- API documentation is generated from Rust contract types and route metadata.

OpenAPI is generated from the Rust API surface using `utoipa` and
`utoipa-axum` for Axum and OpenAPI 3.1. The OpenAPI document is an artifact of
the Rust contract layer, not a separately maintained source file.

## MCP

Tanren's MCP service uses MCP Streamable HTTP as the only product transport.

The MCP server is a network service in the Tanren container stack. Agent
environments call the MCP endpoint over HTTP with scoped credentials and
capability claims. The agent execution environment does not need a local Tanren
binary to mutate Tanren state.

MCP technology rules:

- The MCP endpoint uses the official MCP Streamable HTTP transport.
- MCP messages are JSON-RPC messages validated against Tanren contract types.
- Tool registration is generated or declared from the same contract layer used
  by application services.
- Tool calls emit typed Tanren events through application services.
- Capability checks happen before side effects.
- MCP authentication and authorization share the same identity and policy
  substrate as API clients.
- Logs never write protocol payloads or secret values.

Stdio MCP is not part of the product architecture. Tanren uses one MCP
transport model so deployment, authentication, event streaming, and worker
container topology stay coherent.

## CLI

The CLI is implemented in Rust with `clap`.

The CLI is a human and automation surface for installation, administration,
project workflows, diagnostics, and controlled local operations. It calls the
same command/query service layer as API and MCP operations. It must not maintain
parallel business logic or accept projection-file edits as canonical state.

CLI output rules:

- human output is concise and stable enough for operators;
- machine-readable output is explicit through structured formats;
- secrets are never printed;
- validation and policy errors use the same categories as API and MCP.

## TUI

The TUI is implemented in Rust with Ratatui and Crossterm.

Ratatui owns terminal rendering. Crossterm owns cross-platform terminal input
and backend behavior. The TUI consumes Tanren read models and commands through
the same contract layer as other public interfaces.

The TUI is for laptop/operator workflows: live loop observation, queue and
worker state, runtime target health, incident mode, approvals, project status,
and focused control actions. It is not the phone-capable surface; the
responsive web UI owns that role.

## Responsive Web UI

The web UI is a first-class responsive application built with TypeScript,
React, and Vite.

Web technology rules:

- TypeScript runs in strict mode.
- API types and clients are generated from Tanren's Rust/OpenAPI contract.
- Hand-written duplicate API shapes are forbidden.
- TanStack Query owns server-state fetching, caching, invalidation, mutation
  state, and stale/fresh status in the web app.
- Server state remains server state; the web app must not create a divergent
  client-side source of truth for Tanren resources.
- The UI uses shadcn/ui-style owned components built on Radix primitives and
  Tailwind CSS.
- Icons use a standard icon library rather than custom one-off vector code.
- Accessibility, keyboard navigation, responsive layout, and redaction states
  are product requirements, not polish.

The web app should be optimized for operational product workflows: planning,
status, review, approvals, proof inspection, configuration, integrations,
observation, and recovery. It should not be structured as a marketing site.

## Persistence

Postgres 18.x is Tanren's baseline database for self-hosted container
deployments.

Postgres owns:

- append-only canonical event log;
- event metadata and idempotency records;
- projection checkpoints;
- relational read models;
- job queue tables;
- outbox tables;
- audit records;
- subscriptions and cursor state;
- non-secret credential metadata;
- operational health and usage records.

Every durable state transition is a typed event. Projection tables are derived
from the event log and optimized for current-state queries, filtering, reports,
subscriptions, and interface rendering.

Postgres UUIDv7 support is part of the baseline posture. UUIDv7 identifiers are
preferred for event IDs and durable resource IDs where sortable, globally
unique identifiers improve replay, ordering, and audit behavior.

Database access uses a typed Rust persistence layer. SeaORM and SeaORM
migrations are the accepted baseline unless a subsystem architecture record
replaces them with a stricter typed persistence choice. Raw SQL is allowed where
the persistence layer needs Postgres-specific queueing, locking, projection, or
performance behavior, but raw SQL must stay behind repository/store APIs.

## Queue And Background Work

Postgres is the only required queue and coordination dependency.

Tanren uses Postgres-backed queues for:

- worker dispatch;
- projection work;
- webhook delivery;
- notification delivery;
- scheduled proactive analysis;
- retry and recovery tasks;
- cleanup tasks.

Queue workers claim work using transactional Postgres locking patterns such as
`FOR UPDATE SKIP LOCKED`. Queue rows are durable work records, not ephemeral
messages.

Postgres `LISTEN`/`NOTIFY` may be used as a wakeup signal for workers and
subscription services, but it is not the durable message store. Missed
notifications must be recoverable by querying durable queue, event, outbox, or
cursor tables.

Redis, NATS, Kafka, and external task queues are rejected as required baseline
dependencies. They may be reconsidered only if a specific subsystem outgrows
Postgres-backed coordination and the added operational complexity is justified.

## Realtime And Subscriptions

The API exposes read-model updates through Server-Sent Events first.

SSE is the default for:

- web UI status updates;
- observation feeds;
- notification streams;
- long-running operation progress;
- read-model subscription cursors.

SSE fits Tanren's dominant realtime shape: server-to-client updates over
existing HTTP infrastructure. WebSockets are reserved for workflows that require
true bidirectional realtime sessions and must be justified by the owning
subsystem architecture.

Subscriptions must expose cursor or freshness metadata so clients can recover
from disconnects without treating realtime delivery as canonical state.

## Authentication And Authorization

Tanren supports local authentication and OIDC-compatible external identity.

Local authentication covers self-hosted bootstrap, solo use, and installations
that do not delegate identity. OIDC support allows organizations to connect an
external identity provider without changing Tanren's internal account,
organization, permission, approval, service-account, and audit model.

Authorization is permission-based. Roles are arbitrary grant-time templates
that bundle permissions; access checks resolve against permissions, policy,
scope, and actor identity, not persona labels.

API keys and service accounts are first-class machine identities. They use the
same permission and policy engine as human accounts, with separate lifecycle,
ownership, expiration, attribution, and audit records.

## Secrets

Secret values are encrypted at rest and excluded from event payloads,
projection files, logs, reports, and proof artifacts.

Events and read models may record non-secret metadata:

- secret identity;
- owner scope;
- credential class;
- provider or harness association;
- version;
- presence;
- last updated;
- usage category;
- rotation and revocation state.

The exact encryption envelope, key hierarchy, and rotation mechanics belong in
`subsystems/configuration-secrets.md`. Technology architecture requires only
that secret storage be explicit, encrypted, auditable, and isolated from event
payloads and generated projections.

## Containers And Execution

Tanren ships container images and a Docker Compose baseline.

The Compose baseline includes Postgres, API/web service, MCP service, daemon or
scheduler service, worker service, web UI serving path, and any required
projection/notification/webhook workers. The same images and environment
contracts must be usable under equivalent container orchestrators.

Agent work executes in containers or remote execution targets. The runtime
layer provisions or leases the execution environment, injects scoped access,
runs the selected harness adapter, reports proof outputs, and tears down or retains
resources according to policy.

Unmanaged local worktree execution is not a technology target.

## Harness And Provider Adapters

Codex, Claude Code, and OpenCode are required harness adapter families.

Harness adapters normalize:

- capability reporting;
- assignment execution;
- progress reporting;
- terminal outcome reporting;
- failure classification;
- proof output capture;
- output redaction.

Provider adapters normalize Tanren's calls to external systems such as source
control, CI, issue trackers, cloud/VM providers, identity providers,
notification channels, and webhook infrastructure.

Harness adapters and provider adapters are separate categories. A single
external platform may have both roles.

## Observability

Tanren services emit structured logs, traces, metrics, and audit records.

Observability technology rules:

- Rust services use `tracing` for structured instrumentation.
- Logs are emitted to stdout/stderr according to container conventions, except
  protocol transports that reserve stdout must log to stderr or service logs.
- Secret values and hidden provider details are redacted before persistence or
  emission.
- Trace/request IDs propagate through API, MCP, daemon, worker, projection, and
  integration flows.
- Metrics expose queue depth, projection lag, worker health, runtime target
  health, webhook delivery state, subscription health, and API/MCP request
  health.
- The default self-hosted stack should work without a full telemetry platform,
  while remaining OpenTelemetry-compatible for operators who add one.

Observation product views are not raw observability dashboards. Product
observation views are read models that explain work, risk, quality, provenance,
freshness, uncertainty bounds, and behavior proof in user-visible terms.

## Contract Generation

Tanren's contract layer is Rust-first.

Contract types derive or generate:

- serde serialization;
- JSON Schema where needed;
- OpenAPI request and response schemas;
- TypeScript client types;
- MCP tool schemas;
- CLI structured input/output schemas where applicable.

Generated TypeScript is consumed by the web UI and any bundled client helpers.
The web UI must not define independent copies of API resources or error shapes.

Contract generation failures are build failures.

## Validation And Proof

Local development uses Cargo and `just`.

Canonical commands:

- `just bootstrap` for first-time setup.
- `just install` to install binaries locally.
- `just fmt` for formatting checks.
- `just check` for the fast static gate.
- `just tests` for behavior proof.
- `just ci` for the full PR gate.
- `just fix` for formatting and Clippy auto-fixes.

Repository policy requires full repo gates for validation. Do not use targeted
tests or targeted linting as final proof for changes.

`tests/bdd/` is the executable behavior proof surface. New asserted behavior
requires positive and falsification coverage unless an explicit behavior note
states why that is impossible.

## Accepted Technology Decisions

- Rust is the backend, CLI, TUI, daemon, worker, adapter, and proof language.
- Tokio is the async runtime.
- Axum and Tower are the HTTP API framework and middleware foundation.
- OpenAPI 3.1 is generated from Rust contract types and Axum route metadata
  with `utoipa` and `utoipa-axum`.
- The responsive web UI uses TypeScript, React, Vite, TanStack Query,
  TanStack Router file-based routing, shadcn/ui-style owned components, Radix
  primitives, and Tailwind CSS.
- TypeScript API types are generated with `openapi-typescript`.
- MCP uses Streamable HTTP as the only product transport.
- CLI uses `clap`.
- TUI uses Ratatui and Crossterm.
- Postgres 18.x is the database baseline.
- UUIDv7 is preferred for event IDs and durable resource IDs.
- Postgres-backed queues are the baseline background-work mechanism.
- SSE is the default realtime subscription transport.
- Authentication supports local auth and OIDC-compatible external identity.
- Authorization is permission-based; roles are arbitrary permission templates.
- Secrets are encrypted at rest and excluded from event payloads and
  projections.
- Docker Compose is the baseline container packaging target, without locking
  the system to Compose as the only orchestrator.
- SeaORM migrations are the accepted baseline for schema management, with
  event-log compatibility protected by versioned event decoding.
- OpenTelemetry-compatible instrumentation is required, but the default Compose
  bundle does not require an OpenTelemetry collector or exporter backend.
- Browser notifications use the standard Web Push API where supported, with
  in-app notification read models as the required fallback.
- Primary technology references are `https://docs.rs/utoipa/`,
  `https://openapi-ts.dev/introduction`,
  `https://tanstack.com/router/v1/docs/framework/react/routing/file-based-routing`,
  and `https://opentelemetry.io/docs/collector/install/docker/`.

## Rejected Alternatives

- **Actix Web as the primary API framework.** Rejected despite strong
  performance because Axum's Tower composition and ecosystem fit Tanren's
  contract, middleware, and service-boundary needs better.
- **Warp as the primary API framework.** Rejected because its filter model is
  less aligned with the broader Axum/Tower ecosystem used by Tanren services.
- **Poem as the primary API framework.** Rejected because OpenAPI ergonomics do
  not outweigh Axum's ecosystem fit and middleware model.
- **Stdio MCP as a product transport.** Rejected because Tanren's containerized
  architecture needs a network MCP service shared by agent environments.
- **Redis, NATS, Kafka, or another queue as required baseline infrastructure.**
  Rejected to keep the self-hosted stack operationally simple and
  Postgres-centered.
- **SQLite as the local database.** Rejected because local and team installs
  should share the same Postgres-backed event, projection, queue, and audit
  architecture.
- **Native mobile application as a core technology target.** Rejected for this
  architecture because the responsive web UI is the phone-capable surface.
- **Unmanaged local worktree execution.** Rejected because container and remote
  execution targets give clearer policy, credential, proof, and cleanup
  boundaries.
