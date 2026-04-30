---
schema: tanren.subsystem_architecture.v0
subsystem: configuration-secrets
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Configuration And Secrets Architecture

## Purpose

This document defines Tanren's configuration, credential, and secret
architecture. It is the authority for how configuration is scoped, how
effective settings are resolved, how secret values are stored, how credential
ownership works, how credential use is governed, and how configuration and
secret lifecycle changes become auditable state.

Configuration and secrets exist in every Tanren install. Solo use, team use,
local compose, and larger self-hosted deployments share the same model.

## Subsystem Boundary

The configuration and secrets subsystem owns:

- user, account, organization, project, service-account, and assignment
  configuration records;
- effective configuration resolution;
- configuration inheritance, override, constraint, and lock semantics;
- credential and secret type declarations;
- secret value storage and secret-store adapter contracts;
- credential metadata, versioning, rotation, revocation, and retirement;
- credential-use policy inputs and intended-use bindings;
- secret usage records that never expose values;
- stale, expiring, unused, and overscoped credential detection;
- configuration and secret lifecycle events;
- non-secret repo projection rules for configuration state.

The subsystem does not own identity grants, policy evaluation itself, provider
protocol flows, runtime execution, event append mechanics, or interface
presentation. It provides typed configuration and secret state that those
subsystems consume.

## Core Invariants

1. **Configuration is event-sourced.** Accepted configuration changes,
   credential metadata changes, and secret lifecycle changes are canonical
   events. Effective configuration is a projection.
2. **Secret values are use-only.** Stored secret values are never read back by
   humans or clients. They may be used by authorized subsystems through scoped
   operations.
3. **Secret values are never projected.** Secret values do not appear in
   events, read models, repo files, logs, reports, proof records, runtime output, or
   interface responses.
4. **Secret metadata is not automatically secret.** Names, opaque identifiers,
   owner scope, provider, status, version, usage class, and lifecycle metadata
   may be visible where permissions allow.
5. **Credential ownership is type-specific.** Each credential or secret kind
   declares which ownership scopes are valid. Not every credential can exist at
   every configuration tier.
6. **Configuration resolution is typed.** Tanren does not apply one universal
   precedence ladder to every setting. Each setting family declares its
   inheritance, override, default, and policy-constraint behavior.
7. **Organization policy wins inside organization scope.** Account or user
   preferences may influence ergonomics, but organization policy constrains
   organization-owned projects and shared work.
8. **Credential use is governed by intended use and policy.** Subsystems may
   use configured credentials for declared intended uses. Direct or unusual
   use requires explicit credential-use policy.
9. **Service account credentials belong to service accounts.** The user who
   creates or manages a service account is not the service account.
10. **Configuration history is visible without exposing secrets.** Users can
    inspect who changed settings, which secret metadata changed, and where
    credentials are used without seeing values.

## Configuration Scope Model

Tanren configuration is scoped. Supported configuration scopes are:

- **User**: preferences and personal credentials tied to one human login
  identity, such as personal provider authorization or individual harness
  credentials.
- **Account**: defaults, preferences, provider mappings, and setup choices
  available through an account across personal or administered projects.
- **Organization**: shared defaults, governance constraints, runtime policy
  inputs, provider defaults, and shared secrets for organization-owned work.
- **Project**: repository-specific methodology settings, runtime defaults,
  verification gates, standards locations, provider bindings, webhook settings,
  and project-scoped secrets.
- **Service account**: credentials and settings for a non-human actor.
- **Assignment**: temporary configuration and access references scoped to one
  worker assignment or bounded workflow.
- **Installation**: deployment-level settings required to operate the
  self-hosted Tanren stack.

Configuration scope is part of actor context and policy context. A setting may
be visible, editable, inherited, or constrained depending on identity,
permissions, policy, and the setting family's rules.

## Effective Configuration

Effective configuration is the resolved view used by commands, workers,
integrations, and interfaces. It is a read model derived from configuration
events, identity and policy state, and setting-specific resolution rules.

Effective configuration resolution must explain:

- the setting value or absence;
- the source scope;
- whether the value is inherited, defaulted, overridden, or locked;
- which policy constraint applies, if any;
- whether the value is usable by the current actor;
- freshness or projection position where relevant.

Tanren does not use a single global rule such as "project beats organization
beats account beats user" for all settings. Some settings inherit. Some are
policy constraints. Some are personal preferences. Some are valid only at a
specific scope. Each setting family declares its own resolution behavior.

Organization policy constrains organization-owned work. Account and user
preferences may still affect personal ergonomics, but they cannot override
organization policy for organization projects.

## Setting Families

Configuration is grouped by setting family so ownership and resolution stay
clear. Examples include:

- project methodology and repository layout;
- standards root and standards profile adoption;
- verification gates and proof commands;
- runtime defaults and execution target preferences;
- harness allowlists and harness preferences;
- provider integration defaults;
- notification preferences and routing;
- webhook endpoint configuration;
- credential-use policy inputs;
- budget, quota, maintenance, and incident mode inputs;
- export, retention, and reporting settings.

Subsystem architecture records own the exact setting vocabulary for their
domain. This document owns the scoping, resolution, history, and secret-safety
rules.

## Credential And Secret Ownership

Credential and secret ownership is explicit and type-specific.

Supported ownership modes include:

- **User-owned credential**: access material tied to one user, such as a
  personal source-control token or personal harness credential.
- **Account-owned credential or default**: access material or defaults tied to
  an account for personal or account-administered automation.
- **Project-owned secret**: access material scoped to one project, such as a
  webhook signing secret or repository-specific deployment credential.
- **Organization-owned secret**: shared access material governed at
  organization scope, such as a VM provider API key used for organization
  remote execution targets.
- **Service-account credential**: access material for a non-human service
  account.
- **Worker-scoped temporary access**: short-lived access for one assignment or
  bounded workflow.
- **External secret reference**: a reference to a secret stored in an external
  secret manager while Tanren owns metadata, policy, and usage records.

Each credential or secret kind declares which ownership modes are allowed. For
example, a Git personal access token is user-owned because Tanren should not
encourage organization-wide commits under one shared user's token. A VM
provider API key for organization-managed execution targets may be
organization-owned because environment management is an organization-level
capability.

## Credential Type Registry

Tanren uses a typed credential-kind registry rather than hard-coding every
provider key variant into core architecture.

A credential kind declares:

- stable kind identifier;
- owning subsystem or adapter;
- allowed ownership modes;
- declared capabilities;
- intended uses;
- redaction class;
- rotation and expiry hints;
- validation metadata;
- provider or adapter references where applicable.

Core Tanren defines common credential kinds such as API keys, webhook signing
secrets, worker tokens, generic opaque secrets, and common provider connection
references. Provider and secret-store adapters may register additional
credential kinds, such as a specific cloud API token or app installation
credential, as long as they satisfy the registry contract.

This lets adapters support new providers without requiring core Tanren to
predefine every possible API key shape, while still preserving typed ownership,
capability, intended-use, lifecycle, and redaction behavior.

## Secret Storage

Tanren stores secret metadata canonically through events and read models.
Secret values are stored through a secret-store abstraction.

The standard self-hosted implementation stores encrypted secret values in
Postgres. Encryption uses installation-managed encryption material that is
external to the database and never stored as a database secret value.

The architecture also supports external secret-store adapters, such as
1Password, Vault, cloud secret managers, or organization-managed secret
systems. External secret-store adapters must preserve Tanren's metadata,
permission, lifecycle, usage, audit, and redaction model.

Secret storage rules:

- secret values are write-only/use-only after storage;
- generated API keys or tokens may be displayed once at creation;
- stored API keys and secret values are not recoverable through Tanren;
- rotation creates a new secret version and retires or revokes prior versions
  according to policy;
- secret-store adapter failures surface as typed provider or availability
  failures without leaking values.

## Secret Metadata

Secret metadata is visible according to permissions and redaction policy. It
may include:

- stable secret identity;
- name and description;
- credential kind;
- owner scope;
- responsible actor or service account;
- provider or adapter;
- declared capabilities;
- intended uses;
- current status;
- version number or generation;
- created, rotated, revoked, retired, and last-used timestamps;
- usage counts or usage classes;
- health, expiry, and stale-risk status.

Secret metadata exists so users can understand and govern access without
seeing values.

## Credential Use

Credential use is governed by credential kind, intended-use bindings,
configuration, and policy.

Rules:

- managing a credential and using a credential are separate capabilities;
- subsystems may use configured credentials automatically for their declared
  intended use;
- direct use outside intended use requires explicit credential-use policy;
- workers receive scoped references or temporary access, not broad reusable
  secret values;
- provider integrations and runtime systems request operations through owning
  subsystems where possible instead of reading raw secrets;
- all credential use records identify subsystem, action, scope, actor,
  assignment where relevant, and outcome;
- usage records never include raw environment variables, command arguments, or
  secret values.

For example, if an organization configures a VM provider credential for remote
execution environment management, the environment management subsystem may use
that credential to create and clean up execution targets. That does not imply
that arbitrary workers or users can read or repurpose the same credential.

## Lifecycle

Configuration, credentials, and secrets have explicit lifecycle events.

Configuration lifecycle:

- proposed;
- accepted;
- updated;
- constrained or locked by policy;
- superseded;
- retired.

Credential and secret lifecycle:

- created or imported;
- validated;
- activated;
- rotated;
- suspended;
- revoked;
- expired;
- retired;
- externally unavailable or authorization failed.

Rotation and revocation never expose old or new values. Service account API
keys and generated tokens are displayed once at creation and then managed
through rotate and revoke operations.

## Stale, Expiring, Unused, And Overscoped Access

Tanren tracks credential risk through metadata and usage projections.

Detection may identify credentials that are:

- near expiry;
- expired;
- unused;
- stale relative to active work;
- overscoped for their intended use;
- no longer matched to a configured provider or project;
- failing authorization;
- attached to retired users, service accounts, projects, or organizations.

Suggested actions include rotate, revoke, renew, investigate, narrow scope, or
accept risk where policy permits. Tanren does not revoke credentials
automatically unless an owning policy explicitly authorizes that action.

## Configuration Proposals

Tanren may infer configuration suggestions from repository signals, provider
metadata, failed commands, or user workflows. Examples include proposed
verification gates, standards roots, runtime defaults, project layout
settings, or provider bindings.

Inferred configuration is not accepted configuration. It is represented as a
proposal or recommendation until an authorized actor accepts it through the
owning configuration command. If policy requires review, the proposal remains
pending until approval conditions are satisfied.

## Repo Projections

Non-secret configuration may appear in repo-local projections where useful for
builders and agents. Secret values never appear in repo projections.

Repo projection rules:

- non-secret settings may be generated into docs, project config views, specs,
  standards profiles, or command assets;
- secret identifiers, names, owner scope, status, and intended use may appear
  where permissions and projection type allow;
- secret values, tokens, private keys, webhook signing material, and raw
  provider credentials never appear;
- Tanren-owned configuration projections are regenerated from events and drift
  is remediated through the state subsystem.

## Audit And Events

Configuration and secret state is event-sourced. Events include:

- configuration proposed, accepted, changed, locked, unlocked, superseded, or
  retired;
- effective configuration rebuilt or found stale;
- credential kind registered or updated;
- secret metadata created or changed;
- secret value stored, imported, rotated, revoked, suspended, expired, or
  retired, without including the value;
- secret-store adapter configured or changed;
- credential use allowed, denied, failed, or completed where audit policy
  requires recording;
- stale, expiring, unused, overscoped, or authorization-failed credential
  findings.

Events and audit views preserve who changed configuration, which scope was
affected, which policy was involved, and what downstream work was impacted
without exposing secret values.

## Accepted Configuration And Secrets Decisions

- Configuration is event-sourced and effective configuration is projected.
- Configuration resolution is typed by setting family rather than governed by
  one universal precedence ladder.
- Organization policy constrains organization-owned work.
- Credential and secret ownership is type-specific.
- Credential kinds declare allowed ownership modes, capabilities, intended
  uses, rotation hints, and redaction behavior.
- Provider and secret-store adapters may register credential kinds through a
  typed registry contract.
- The standard self-hosted secret store is encrypted Postgres secret values
  using installation-managed encryption material external to the database.
- External secret-store adapters are supported by architecture.
- Secret values are write-only/use-only after storage.
- Generated API keys and tokens may be displayed once at creation but are not
  recoverable afterward.
- Managing a credential and using a credential are separate capabilities.
- Subsystems may use configured credentials automatically for their declared
  intended use.
- Workers receive scoped references or temporary access rather than broad
  reusable secret values.
- Secret usage records include subsystem, action, assignment, scope, and
  outcome but never raw environment variables, command arguments, or values.
- Non-secret configuration and secret metadata may be projected where
  permitted; secret values are never projected.
- Core credential kinds are local password credentials, API keys, MCP tokens,
  webhook signing keys, worker assignment tokens, OIDC client configuration,
  source-control user credentials, source-control service credentials, and
  secret-store master-key references. Provider adapters register additional
  provider-specific kinds.
- Secret-store adapter operations use a typed envelope containing credential
  kind, owner scope, secret reference, version, intended use, policy context,
  actor, correlation ID, redaction class, and result metadata.
- Policy locks are required before v1 for identity, authorization, provider
  connections, runtime placement, budget/quota, standards profiles, webhook
  delivery, and secret-store configuration.
- Credential create, rotate, revoke, failed use, privileged use, worker
  issuance, provider action use, and raw export use are always audited.
  Routine successful use may be summarized in usage projections.
- The Postgres secret store backup model exports encrypted values only with
  key manifest metadata; restore requires explicit key availability or
  rotation into new installation-managed encryption material.

## Rejected Alternatives

- **One universal configuration precedence ladder.** Rejected because settings,
  policies, credentials, and personal preferences have different ownership and
  resolution semantics.
- **Every credential valid at every scope.** Rejected because some credentials
  are inherently personal while others are inherently project,
  organization, service-account, or assignment scoped.
- **Secret values in events or projections.** Rejected because events and
  projections are durable, replayable, exportable, and broadly consumed.
- **Readable stored secrets.** Rejected because Tanren secrets are for
  governed use, not value recovery.
- **User-owned API keys as service accounts.** Rejected because non-human
  automation needs independent actor identity and lifecycle.
- **External secret managers replacing Tanren metadata.** Rejected because
  Tanren still owns lifecycle, policy, usage, audit, and projection semantics.
- **Manual wiring for every intended credential use.** Rejected because
  configured subsystem credentials should be usable for their declared purpose
  without forcing brittle per-workflow setup.
