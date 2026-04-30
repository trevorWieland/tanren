---
schema: tanren.delivery_architecture.v0
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Delivery Architecture

## Purpose

This document defines how Tanren is installed, packaged, bootstrapped,
upgraded, and removed. Delivery is about making the Tanren control plane and
its generated repository assets available in a repeatable self-hosted form.

Delivery is not the subsystem that delivers product specs. Spec execution,
pull requests, reviews, walks, merge readiness, source-control operations, and
release learning are owned by orchestration, provider integrations,
observation, and assessment.

Delivery owns the installable shape of Tanren itself:

- container images;
- Docker Compose baseline;
- orchestrator-neutral service contracts;
- first-run stack bootstrap mechanics;
- deployment bundles;
- generated repository asset materialization;
- generated harness integration assets;
- standards profile projection;
- projection drift checks and remediation;
- upgrade and migration packaging;
- uninstall mechanics.

## Architecture Boundary

Delivery owns:

- release bundle structure;
- container image contracts;
- required service names and entrypoints;
- Docker Compose baseline files;
- environment variable and volume contracts;
- healthcheck and readiness contracts;
- stack bootstrap commands and documentation;
- first-run setup entrypoints;
- infrastructure-as-code file discovery and application mechanics;
- repo bootstrap materialization mechanics;
- generated command and harness assets;
- generated MCP and API connection assets for harnesses;
- standards profile materialization;
- install preview and drift preview;
- upgrade bundle and migration invocation mechanics;
- stack uninstall mechanics;
- repo asset uninstall mechanics.

Delivery does not own:

- product, behavior, roadmap, architecture, or spec state semantics;
- source-control provider API mechanics;
- pull request lifecycle semantics;
- runtime target provisioning;
- backup, restore, disaster recovery, maintenance, or incident runbooks;
- provider credentials or secret storage;
- identity-policy semantics;
- configuration semantics;
- public API/MCP contract semantics;
- operation of webhook or notification delivery.

Delivery packages and invokes capabilities that other subsystems define. It
does not create alternate local paths around those subsystems.

## Core Invariants

1. **Self-hosted is the product delivery model.** Tanren ships as
   self-hostable open-source infrastructure. Managed hosting is the same stack
   plus external commercial and operating layers outside this repo.
2. **Local use is small self-hosted use.** Local usage uses the same
   conceptual services, Postgres state model, HTTP API, HTTP MCP, identity,
   policy, projections, and workers as team usage.
3. **No local-only backend.** Delivery must not introduce SQLite-only,
   no-auth, direct-file, or direct-service bypasses for product state.
4. **Compose is the baseline, not the lock-in.** Docker Compose is the primary
   supported delivery bundle, but images, environment variables, volumes, and
   service contracts must be usable under equivalent orchestrators.
5. **Services are explicit.** The baseline stack uses named services for API,
   web, MCP, daemon/scheduler, workers, projection/outbox workers, and
   Postgres instead of hiding the product behind a single local process.
6. **All public surfaces are valid after bootstrap.** Web, API, MCP, CLI, and
   TUI interact with the same running control plane and event log.
7. **Repository onboarding is source-control based.** Tanren bootstraps a repo
   by creating and managing a source-control branch/PR, not by inventing local
   state in a checkout.
8. **Repo-local assets are projections or controlled install artifacts.**
   Tanren-owned docs, specs, command assets, harness config, standards
   profiles, and metadata are generated from typed state.
9. **Drift is not accepted silently.** Tanren-owned generated assets are
   checked for drift and remediated by regenerating from canonical state.
10. **Stack uninstall and repo uninstall are separate.** Removing a Tanren
    installation and removing Tanren assets from a project repository are
    independent flows.

## Installable Stack

The installable Tanren stack contains these services:

- `tanren-api`: HTTP API for first-party and external clients;
- `tanren-web`: responsive web UI served against the API;
- `tanren-mcp`: MCP Streamable HTTP service for agents and harnesses;
- `tanren-daemon`: scheduling, reconciliation, lifecycle, and background
  coordination;
- `tanren-runtime-worker`: execution assignment workers;
- `tanren-projection-worker`: projection and read-model workers;
- `tanren-provider-worker`: provider action and reconciliation workers;
- `tanren-webhook-worker`: webhook delivery and retry workers;
- `tanren-notification-worker`: notification delivery workers;
- `postgres`: canonical event log, projections, read models, audit records,
  secrets metadata, and encrypted secret values;
- optional reverse proxy, TLS, and object/log storage components where an
  operator chooses to include them.

The exact process split may evolve, but the service responsibilities are part
of the delivery contract. Operators may co-locate compatible service processes
inside an image or orchestrator task only if public service contracts,
readiness, logging, health, and scaling semantics remain clear.

## Deployment Bundle

Tanren releases include a versioned deployment bundle.

The bundle includes:

- Compose files;
- `.env.example`;
- service names;
- image references and tags;
- volume declarations;
- network declarations;
- healthcheck definitions;
- migration invocation entrypoints;
- upgrade notes;
- bootstrap instructions;
- comments explaining which values are installation-local;
- optional templates for reverse proxy and TLS integration.

The bundle is inspectable and operable without a hidden CLI runtime. A CLI may
generate, copy, validate, or manage the bundle, but the bundle itself remains
plain deployment material.

## Compose Baseline

Docker Compose is the primary baseline because it gives local and team installs
a repeatable, inspectable, containerized shape.

The Compose baseline must provide:

- explicit services;
- persistent Postgres volume;
- stable service DNS names;
- healthchecks;
- restart policies suitable for local/team use;
- environment variable configuration;
- image tags pinned to a Tanren release;
- logs emitted to stdout/stderr;
- no dependency on a host-local database;
- no dependency on a host-local source checkout for product execution.

Equivalent deployment through Podman, Kubernetes, Nomad, or another
orchestrator must be able to run the same images and environment contracts.

## First-Run Stack Bootstrap

First-run stack bootstrap creates a running Tanren installation.

The preferred pattern is:

1. obtain the deployment bundle;
2. configure `.env` or equivalent orchestrator values;
3. start the stack;
4. wait for API, web, MCP, workers, and Postgres readiness;
5. create or select the first installation administrator;
6. apply optional declarative configuration;
7. connect providers, projects, and harnesses through Tanren actions.

First-run setup is available through web, API, CLI, and TUI once the stack is
running. These surfaces all call the same control-plane API and append the same
typed events.

The CLI may provide stack helper commands, such as initializing a deployment
bundle or validating environment values. Such commands must be clearly named
as stack installation helpers and must not be confused with repository or
product-state setup.

## CLI And TUI Role

CLI and TUI are valid operator and power-user clients.

They may:

- initialize or validate a deployment bundle;
- start, stop, or inspect a local Compose stack where configured;
- perform first-run setup through the API;
- apply declarative configuration through the API;
- connect projects and providers through the API;
- request repo bootstrap actions through the API;
- inspect drift, health, and migration state;
- trigger controlled projection regeneration.

They must not:

- mutate Tanren product/project state by editing local files as canon;
- bypass the HTTP control plane after the stack exists;
- use private application-service APIs;
- create no-auth local product state;
- treat local checkout writes as an alternate repository bootstrap model.

## Declarative Configuration And IaC

Delivery supports infrastructure-as-code and configuration-as-code mechanics.

Declarative files may describe:

- installation defaults;
- account and organization defaults;
- project definitions;
- source-control repository bindings;
- provider connection metadata;
- standards profile selection;
- harness allowlists;
- runtime placement defaults;
- policy templates;
- service account declarations;
- webhook endpoint declarations;
- observation report and digest settings;
- non-secret configuration.

Delivery owns how these files are discovered, validated as install inputs, and
applied through the control plane. Identity-policy, configuration-secrets,
provider integrations, client integrations, runtime, and other subsystems own
the semantics of the declarations.

Secret values must not be embedded in declarative files by default.
Declarative configuration references secret bindings, external secret manager
locations, one-time import inputs, or installation-managed secret records.

Applying declarative configuration is an event-producing Tanren action, not a
direct database write or file-only mutation.

## Project And Repository Bootstrap

Project bootstrap starts from a running Tanren installation and a configured
source-control provider.

The preferred and core architecture path is:

1. a user imports or creates a Tanren project for one source-control
   repository;
2. Tanren verifies the source-control provider, repository access, branch
   permissions, and mergeability;
3. Tanren creates a managed bootstrap branch;
4. the user proceeds through Tanren planning actions, such as product planning,
   behavior identification, architecture planning, standards selection, and
   roadmap shaping;
5. Tanren materializes repo-local projections and harness assets onto the
   bootstrap branch;
6. the bootstrap branch is reviewed through source-control provider workflow;
7. once accepted, the bootstrap branch merges through the same source-control
   path as other Tanren-managed work.

This makes repository onboarding reviewable and compatible with normal branch
protection. It also prevents local checkout state from becoming a hidden
source of truth.

Local checkout writes are not the core product path. If an operator path exists
for writing generated assets into a checkout, it must authenticate to the
control plane, request projection materialization, and preserve Tanren's source
of truth in the event log.

## Generated Repository Assets

Tanren may materialize repo-local assets for human and agent readability.

Generated or controlled assets include:

- product projections;
- behavior projections;
- architecture projections;
- roadmap projections;
- spec and task projections;
- behavior-proof summaries and indexes;
- standards profile files;
- harness command assets;
- harness MCP/API connection config;
- repo-local Tanren metadata;
- read-only projection manifests.

Each generated asset records enough metadata to identify:

- owning Tanren installation or project;
- projection type;
- source event position or projection version;
- schema version;
- generation timestamp;
- drift policy;
- whether the file is Tanren-owned or user-owned input.

Manual edits to Tanren-owned projections are drift. Changes should happen
through Tanren actions, imports, or approved editing surfaces that emit typed
events and regenerate projections.

## Harness Asset Generation

Tanren generates harness-specific assets for Codex, Claude Code, and OpenCode.

Harness assets exist because each harness needs local command or skill
registration, MCP connection configuration, and scoped access setup. The
primary product contract remains HTTP MCP and API; harness files are
projections that connect each harness to those contracts.

Harness assets may include:

- command or skill definitions;
- tool-use instructions;
- MCP server connection settings;
- API endpoint references;
- capability or token bootstrap references;
- redaction and scope instructions;
- projection metadata.

Harness assets are Tanren-owned controlled projections. Reinstalling or
regenerating them replaces stale Tanren-owned content while preserving
unrelated user-owned files according to the declared merge policy.

## Standards Profiles

Standards profiles are Tanren-owned projections once selected for a project.

Tanren ships multiple opinionated default standards profiles. Users can select
profiles during bootstrap, clone profiles, import standards files, edit
standards through Tanren actions, and remove standards through controlled
Tanren actions.

Standards editing must be easy, but it still flows through Tanren:

- direct edits through web, CLI, TUI, API, or MCP emit typed events;
- imports validate source files and emit typed events;
- generated standards projections are regenerated from accepted state;
- manual edits to generated standards files are drift.

This keeps standards machine-readable, auditable, and available to runtime
gates and harnesses without making repo-local files the source of truth.

## Projection Drift

Delivery owns install and projection drift checks for generated assets.

Drift detection compares generated artifacts against the projection Tanren
expects from canonical state.

Drift states include:

- `clean`;
- `missing`;
- `modified`;
- `stale`;
- `unknown_owner`;
- `blocked_by_user_file`;
- `regeneration_required`.

Drift checks can run as preview actions, source-control checks, operator
diagnostics, or controlled remediation. Drift remediation regenerates
Tanren-owned assets from canonical state. Drifted files must not be silently
accepted as source input.

When a user wants to turn a manual file change into canonical state, they use a
typed import or edit action that validates the change and emits events.

## Install Preview

Delivery supports previews for stack setup, repo bootstrap, projection
materialization, standards profile changes, harness asset changes, upgrades,
and uninstall.

A preview shows:

- files or resources to be created, updated, or removed;
- services affected;
- volumes affected;
- migration steps;
- generated asset changes;
- drift remediation actions;
- destructive effects;
- required permissions;
- policy denials;
- warnings and unsupported configuration.

A preview does not create a dev-only path. Applying the preview still happens
through the control plane or deployment mechanism that owns the real action.

## Upgrades And Migrations

Delivery owns packaging the upgrade mechanism.

Upgrade packages include:

- new image tags;
- updated Compose or deployment bundle files;
- database migration binaries or entrypoints;
- projection migration or regeneration steps;
- compatibility notes;
- public contract compatibility notes;
- preflight checks;
- rollback constraints.

Operations owns production upgrade runbooks, backup-before-upgrade policy,
maintenance mode, restore validation, and incident handling.

Migrations are run through explicit delivery entrypoints or in-application
upgrade capabilities that still obey the same event, audit, and policy model.
Automatic migration behavior must be visible and controllable enough for
self-hosted operators.

## Stack Uninstall

Stack uninstall removes or disables a Tanren installation.

Stack uninstall may affect:

- containers;
- images;
- networks;
- volumes;
- database data;
- local deployment bundle files;
- logs or diagnostic files;
- installation secrets.

Data deletion must be explicit. Stopping or removing containers is not the same
as deleting Postgres volumes, backups, secret material, or exported data.

Stack uninstall does not automatically remove Tanren assets from repositories.
A project repo may still contain generated docs, standards, harness files,
spec projections, or metadata after the stack is removed.

## Repo Uninstall

Repo uninstall removes Tanren-managed assets from a project repository without
deleting user-owned work.

Repo uninstall should use a source-control branch/PR where possible.

It may remove:

- generated command and harness assets;
- generated MCP/API harness config;
- generated Tanren metadata;
- generated standards projections where policy allows;
- generated docs, specs, roadmap views, proof summaries, and projection
  manifests where policy allows.

Repo uninstall must preview destructive effects before applying them. It must
not delete user-owned source code, tests, product code, or unrelated project
files.

Repo uninstall does not remove the Tanren stack, database, provider
connections, accounts, organizations, or project history unless separate
Tanren actions explicitly do so.

## Security And Policy

Delivery actions require permission checks.

Security requirements:

- stack bootstrap secrets are not committed;
- generated assets redact secrets and hidden provider details;
- harness config uses scoped credentials or references, not broad permanent
  secrets;
- repo bootstrap branches are created through source-control provider policy;
- repo asset materialization respects project and repository permissions;
- destructive uninstall actions require explicit confirmation and audit;
- IaC import validates scope and permission before applying changes;
- migration and upgrade actions are auditable;
- delivery helpers must not bypass identity-policy, configuration-secrets, or
  provider integration boundaries.

## Events

Delivery emits typed events for:

- deployment bundle initialized or validated;
- stack first-run setup started or completed;
- delivery preflight completed;
- declarative configuration applied;
- repo bootstrap started, materialized, reviewed, merged, failed, or cancelled;
- generated asset projection materialized;
- harness assets materialized;
- standards profile materialized;
- drift check completed;
- drift remediation requested or completed;
- upgrade preflight completed;
- migration started, completed, failed, or rolled back where supported;
- stack uninstall previewed or completed;
- repo uninstall previewed or completed.

Events may include file paths, projection identifiers, resource references, and
redacted diagnostics. Events must not include secret values.

## Read Models

Required delivery read models include:

- stack install status;
- service image and version status;
- deployment bundle version;
- first-run setup status;
- declarative configuration application status;
- repo bootstrap status;
- generated asset inventory;
- harness asset inventory;
- standards profile projection status;
- projection drift status;
- upgrade readiness;
- migration status;
- stack uninstall preview;
- repo uninstall preview;
- delivery audit history.

## Accepted Decisions

- Delivery owns installation, packaging, generated assets, upgrades, uninstall,
  and projection drift, not product spec delivery.
- Docker Compose is the primary baseline, without locking the architecture to
  Compose.
- The baseline stack uses separate named services.
- Local usage is the same self-hosted architecture in a smaller profile.
- There is no local-only or no-auth backend path for product state.
- First-run setup can be performed through any public surface after the stack
  is running.
- Declarative configuration and IaC are first-class delivery inputs.
- Repo bootstrap is a source-control managed branch/PR workflow.
- Generated repo-local assets are Tanren-owned projections or controlled
  install artifacts.
- Codex, Claude Code, and OpenCode harness assets are generated even though
  the product contract is HTTP MCP/API.
- Standards profiles are Tanren-owned projections, with easy edit and import
  actions through Tanren.
- Stack uninstall and repo uninstall are separate flows.
- Stable Compose service names are `postgres`, `tanren-api`, `tanren-web`,
  `tanren-mcp`, `tanren-daemon`, `tanren-runtime-worker`,
  `tanren-projection-worker`, `tanren-provider-worker`,
  `tanren-webhook-worker`, `tanren-notification-worker`, and one-shot
  migration/bootstrap jobs.
- Caddy is the primary reverse-proxy and TLS example. Nginx is a secondary
  example.
- CLI stack helpers are `stack init`, `stack doctor`, `stack upgrade`,
  `stack backup`, `stack restore`, and `stack uninstall`.
- CLI project helpers are `project import`, `project bootstrap`, and
  `project uninstall`.
- The first IaC schema supports installation, account, organization, project,
  provider connection, standards profile, policy, secret metadata, and schedule
  resources.
- Default standards profiles are `balanced`, `strict-production`,
  `local-utility`, and `library`.
- Stable harness asset paths are `.codex/`, `.claude/`, and `.opencode/`.
- Before v1, migrations may make breaking changes with explicit reset/export
  guidance. At v1 and later, migrations preserve event-log compatibility.
- Repo uninstall removes Tanren-generated harness/config projections by
  default and retains planning/proof history unless explicitly selected.

## Rejected Alternatives

- **Local-only delivery mode.** Rejected because local and team usage should
  share the same control-plane, Postgres, identity, policy, and projection
  architecture.
- **CLI as an alternate backend.** Rejected because CLI and TUI are clients of
  the running control plane, not a bypass around API, events, policy, or
  projections.
- **Repo bootstrap by local checkout mutation.** Rejected because project
  onboarding should be reviewable through source control and rooted in Tanren
  events.
- **Stdio MCP as the product delivery path.** Rejected because Tanren's
  containerized architecture uses HTTP services, and execution targets should
  not require a local Tanren binary.
- **Repo-local generated files as source of truth.** Rejected because Tanren
  state is canonical in the event log and generated files are projections.
- **Manual standards file edits as canonical.** Rejected because standards need
  typed state, auditability, imports, and regeneration for runtime and harness
  use.
- **One combined local service as the baseline.** Rejected because explicit
  services make readiness, logs, scaling, workers, and failure boundaries clear
  from the start.
- **Stack uninstall also removing repo assets.** Rejected because installation
  lifecycle and repository lifecycle are independent.
