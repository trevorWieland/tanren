---
schema: tanren.operations_architecture.v0
status: accepted
owner_command: architect-system
updated_at: 2026-04-30
---

# Operations Architecture

## Purpose

This document defines how a running Tanren installation is operated safely.
Operations keeps Tanren healthy, recoverable, governable, auditable, and
controllable under normal use, maintenance, incidents, quota pressure, provider
failure, and recovery events.

Operations does not package or install Tanren. Delivery owns container images,
deployment bundles, Compose baseline, generated assets, upgrade packaging, and
uninstall mechanics. Operations owns the operator controls and runbook
semantics for a live installation.

## Architecture Boundary

Operations owns:

- operational health model;
- maintenance mode;
- incident mode;
- installation safe mode;
- pause and resume controls by scope;
- drain and retire controls from an operator perspective;
- backup/export jobs;
- restore/import jobs;
- disaster recovery validation;
- operational audit export;
- cost, quota, and budget operating state;
- scheduled job cadence, run windows, retries, and pause/resume;
- production upgrade runbook policy;
- retention policy for operational artifacts;
- operational reconciliation after interruption;
- operator-facing runbook states and recovery actions.

Operations does not own:

- deployment bundle format or image build mechanics;
- runtime placement implementation;
- execution target provisioning mechanics;
- provider API calls;
- source-control and PR lifecycle mechanics;
- behavior-proof semantics;
- assessment finding semantics;
- observation dashboard composition;
- identity-policy semantics;
- credential or secret storage;
- public API/MCP interface contracts.

Operations invokes and coordinates other subsystems through the same API,
event, policy, and worker model as any other Tanren subsystem.

## Core Invariants

1. **Operations is event-sourced.** Operational mode changes, exports,
   restores, validation runs, quota transitions, pauses, resumes, drains, and
   audit exports are durable events.
2. **No shell escape hatch becomes product behavior.** Operators may use shell
   tools to manage containers, but Tanren-controlled state changes go through
   authenticated Tanren commands.
3. **Operational controls are scoped.** Modes and controls apply to explicit
   installation, account, organization, project, provider, runtime pool,
   execution target, worker group, schedule, or job scopes.
4. **Parent policy can constrain children.** Higher-scope operational controls
   apply to child scopes unless an explicit, permitted exemption exists.
5. **Maintenance and incident mode are distinct.** Maintenance mode is planned
   and runbook-driven. Incident mode is reactive and safety-driven.
6. **Incident mode is not automatically global stop.** Incident effects are
   selected by scope and policy, because not every incident requires stopping
   unrelated work.
7. **Safe mode exists for serious installation risk.** Safe mode makes the
   installation read-mostly, pauses workers and provider actions, and leaves
   only recovery/admin operations available.
8. **Restore requires preview.** Restore jobs show what will be created,
   replaced, omitted, or blocked before mutation.
9. **Missing cost or quota data is not zero.** Cost, quota, and provider usage
   reports preserve provenance and expose unknown or unsupported inputs.
10. **Audit exports never include secrets.** Secret values and hidden payloads
    remain redacted in operational exports.

## Operational Scope Model

Operational controls can target:

- installation;
- account;
- organization;
- project;
- roadmap or spec scope where applicable;
- runtime pool;
- execution target;
- worker group;
- provider connection;
- client integration;
- schedule;
- operational job.

Controls are hierarchical where the resource model is hierarchical. For
example, an organization pause affects projects in that organization unless
policy explicitly allows an exemption and an authorized actor grants one.

Exemptions are events. They identify the parent control, exempted child scope,
actor, reason, expiration, and policy basis.

## Operational Modes

Operations supports normal mode, maintenance mode, incident mode, and safe
mode.

### Normal Mode

Normal mode allows work to proceed according to planning, orchestration,
runtime, provider, client, identity, and policy state.

### Maintenance Mode

Maintenance mode is planned. It is used for upgrades, migrations, provider
changes, infrastructure work, backup validation, policy review, or other known
operational windows.

Maintenance mode records:

- scope;
- reason;
- expected start and end;
- allowed operations;
- restricted operations;
- notification behavior;
- affected queues, workers, provider actions, and schedules;
- exit criteria.

Maintenance mode may pause new work, drain targets, pause schedules, require
extra approvals, or suppress noncritical notifications according to policy.

### Incident Mode

Incident mode is reactive. It is used when Tanren or a dependency is degraded,
unsafe, compromised, over quota, or behaving unexpectedly.

Incident mode records:

- scope;
- severity;
- reason;
- suspected cause where known;
- selected effects;
- affected resources;
- owner or responder;
- update history;
- exit criteria.

Incident effects are selectable:

- observe only;
- pause new work;
- pause selected active work;
- drain execution targets;
- suspend provider actions;
- pause schedules;
- revoke or narrow worker access;
- require additional approvals;
- suppress noncritical notifications;
- cancel selected assignments;
- block selected client or provider operations.

Incident mode does not bypass audit and does not hide incidents from affected
users with visibility.

### Safe Mode

Safe mode is an installation-level recovery posture for serious operational
risk.

Safe mode defaults:

- API is read-only except explicit admin recovery actions;
- web, CLI, TUI, and MCP expose safe-mode status;
- new work is paused;
- execution workers stop claiming work;
- provider actions are paused unless needed for recovery;
- webhook and notification delivery may be paused or restricted;
- scheduled jobs are paused;
- worker-scoped access is revoked or allowed to expire;
- restore, export, audit, diagnostic, and recovery commands remain available
  only to permitted operators.

Safe mode is useful for suspected compromise, dangerous migration failure,
database corruption suspicion, severe provider failure, or operator-directed
recovery.

## Pause, Resume, Drain, And Retire

Pause and resume controls govern work intake and operational actions.

Pause controls may target:

- new roadmap/spec work;
- new runtime assignments;
- selected active work;
- provider actions;
- webhook delivery;
- notification delivery;
- scheduled jobs;
- client writes;
- project or organization work intake.

Resume lifts a pause only when policy allows and unresolved parent controls do
not still block the scope.

Drain prevents new assignments from landing on a runtime pool, worker group, or
execution target while in-flight work follows configured policy.

Retire removes a runtime pool, worker group, or execution target from future
placement. Retire does not delete historical execution records.

Cancelling in-flight work is an explicit action, not the default consequence of
pause, incident, maintenance, drain, or retire.

## Health And Reconciliation

Operations owns the operational health model for the running installation.

Health sources include:

- service readiness;
- API, web, MCP, daemon, worker, projection worker, and outbox worker status;
- queue depth and age;
- lease age and ownership;
- projection lag;
- Postgres connectivity;
- provider connection health;
- execution target health;
- schedule status;
- webhook and notification delivery health;
- backup/export/restore job status;
- migration status;
- budget and quota status.

Health states include:

- `normal`;
- `degraded`;
- `unavailable`;
- `draining`;
- `paused`;
- `maintenance`;
- `incident`;
- `safe_mode`;
- `unknown`.

Operations reconciliation jobs detect stuck leases, orphaned jobs, drifted
operational state, unfinished exports, ambiguous restores, stalled queues,
missed schedules, and inconsistent health reports. Reconciliation emits events
and routes semantic repair to the owning subsystem.

## Backup And Export

Operations supports point-in-time export for installation, account, and project
scopes.

Export artifacts include:

- canonical event/state data for the selected scope;
- schema versions;
- export manifest;
- resource identifiers;
- projection metadata sufficient to regenerate repo-local projections;
- configuration metadata visible to the exporter;
- non-secret provider, client, and runtime references;
- redaction and omission summary;
- artifact fingerprint;
- export timestamp and source installation metadata where useful.

Export artifacts do not include secret values, hidden user-tier credentials, or
data outside the actor's visibility. Repo-local projections are generally
recoverable from git and projection metadata; they are not backup canon.

Source metadata may be stored for operator traceability, artifact
deduplication, and migration diagnostics. Restores do not depend on preserving
original event positions unless performing a full installation restore.

## Restore And Import

Operations supports restoring from export artifacts.

Restore targets include:

- new account seeded from account export;
- empty project;
- inactive existing project selected for overwrite;
- full installation restore where an operator is rebuilding an installation.

Restore preview shows:

- target scope;
- created resources;
- replaced resources;
- omitted resources;
- conflicts;
- schema compatibility;
- required permissions;
- active work blocks;
- redactions and unavailable secrets;
- provider or credential relinking required after restore.

Project overwrite is allowed only when the target is inactive and the actor
explicitly confirms replacement. Restores into active projects with running
work are rejected.

For project/account migration into an existing installation, imported events
receive new local event positions. Original source identifiers may remain in
the import manifest or metadata for diagnostics, but local canon is the
restored installation's event log.

For full installation disaster recovery, operators may restore database-level
backups or replay full-installation artifacts according to the recovery
runbook.

## Disaster Recovery Validation

Disaster recovery validation checks that recovery paths are usable before an
incident.

Validation may inspect:

- backup/export recency;
- artifact readability;
- schema compatibility;
- restore permissions;
- target availability;
- secret relinking requirements;
- provider reconnection requirements;
- database backup metadata;
- expected recovery procedure;
- recent validation result.

DR validation is non-destructive by default. Destructive validation requires an
explicit isolated target and permission.

Validation results are operational records. They are source signals for
observation and can route findings to assessment when they reveal product,
architecture, provider, or operational gaps.

## Cost, Quota, And Budget Operations

Operations owns cost, quota, and budget operating state. Identity-policy owns
who may configure limits. Provider integrations supply provider usage signals.
Observation renders summaries and trends.

Operations supports:

- warning thresholds;
- hard-stop thresholds;
- per-scope budgets;
- provider quota awareness;
- harness usage tracking;
- execution target live-time tracking;
- queue and worker usage tracking;
- visible unknown or unsupported provider cost data;
- budget and quota actions.

Threshold actions include:

- warn only;
- require approval for new work;
- pause new work;
- pause provider actions;
- drain runtime pools;
- block specific harnesses or target classes;
- enter incident mode for the affected scope.

Hard-stop thresholds must be policy-backed and visible to affected users.

## Scheduled Jobs

Operations owns the scheduling substrate for recurring or delayed work.

Schedules define:

- job kind;
- owning subsystem;
- scope;
- cadence or trigger;
- run window;
- retry policy;
- timeout;
- pause/resume state;
- backoff behavior;
- last run state;
- next run state.

Operations owns when jobs run, how missed schedules are reconciled, and how
schedules pause during maintenance, incident, safe mode, or quota pressure.

The owning subsystem defines job semantics. Assessment owns assessment meaning,
behavior proof owns proof meaning, observation owns digest content, delivery
owns migration package mechanics, and provider/client integrations own external
delivery mechanics.

## Upgrade Operations

Delivery packages upgrade and migration entrypoints. Operations owns the
production runbook policy around using them.

Upgrade operations include:

- preflight checks;
- backup-before-upgrade policy;
- maintenance-mode recommendation;
- maintenance-mode requirement when preflight detects unsafe concurrent work;
- migration execution status;
- projection regeneration status;
- rollback constraints;
- compatibility warnings;
- post-upgrade health checks.

Operations may require safe mode or maintenance mode for high-risk migrations,
schema changes, or recovery from failed upgrade attempts.

## Operational Audit

Operations exports normalized operational audit views by default.

Audit exports include attributed records for:

- operational mode changes;
- pause/resume/drain/retire actions;
- backup/export/restore jobs;
- DR validation;
- migration and upgrade operations;
- budget and quota transitions;
- placement and approval decisions;
- provider action summaries;
- worker and runtime operational events;
- credential and policy lifecycle metadata where visible;
- integration and webhook operational events.

Raw event-stream export is a separate high-permission export path. It is useful
for migration, forensic analysis, or complete installation backup, but it is
not the default operational audit export.

Audit exports include redaction and omission summaries. They never include
secret values.

## Retention

Operations owns retention policy for operational artifacts.

Retention policy may cover:

- export artifacts;
- restore artifacts;
- audit export artifacts;
- bounded runtime output;
- webhook delivery diagnostics;
- provider diagnostics;
- schedule run history;
- health snapshots;
- incident records;
- maintenance records;
- migration diagnostics.

Retention policy must distinguish canonical events from derived artifacts.
Canonical events are append-only. Derived operational artifacts may expire,
archive, or be deleted according to policy.

## Security And Policy

Operational actions require permission checks and audit.

Security requirements:

- operational controls respect scope and parent policy;
- incident and maintenance modes do not bypass audit;
- safe mode preserves recovery access without exposing broad mutation rights;
- export and restore enforce visibility and restore-target policy;
- secrets are never exported unless explicitly supported as encrypted,
  installation-level recovery material under a separate high-permission path;
- audit exports redact secrets and hidden resources;
- cost and quota summaries do not reveal hidden scopes;
- scheduled jobs run as explicit system or service-account actors;
- operational reconciliation cannot silently mutate product state owned by
  other subsystems.

## Events

Operations emits typed events for:

- operational mode entered, updated, or exited;
- safe mode entered or exited;
- pause requested, applied, denied, expired, or lifted;
- resume requested, applied, or denied;
- drain requested, started, completed, or cancelled;
- target retired or restored to service;
- health state changed;
- reconciliation run started, completed, or failed;
- export requested, started, completed, failed, cancelled, or expired;
- restore preview generated;
- restore started, completed, failed, cancelled, or rolled back where
  supported;
- DR validation started, completed, or failed;
- budget or quota threshold warning reached;
- budget or quota hard stop reached or cleared;
- schedule created, updated, paused, resumed, fired, missed, retried, failed,
  or completed;
- upgrade preflight started, completed, or failed;
- migration started, completed, or failed;
- operational audit export requested, completed, failed, or accessed.

Events may include artifact identifiers, resource references, policy
references, and redacted diagnostics. Events must not include secret values.

## Read Models

Required operations read models include:

- operational mode by scope;
- safe-mode status;
- pause/resume state by scope;
- drain and retire state;
- service and worker health;
- queue and lease health;
- execution target operational health;
- projection and outbox health;
- backup/export job status;
- restore preview and restore job status;
- disaster recovery readiness;
- cost, quota, and budget state;
- schedule catalog and run history;
- upgrade and migration readiness;
- operational audit export history;
- operational reconciliation history.

Read models include freshness, scope, actor visibility, redaction, and source
position metadata where applicable.

## Accepted Decisions

- Operations owns live-installation control, recovery, audit, health,
  schedules, and production runbook semantics.
- Delivery owns packaging and migration entrypoints; operations owns when and
  how those entrypoints are safely used.
- Operational modes are scoped and hierarchical.
- Incident mode supports selectable effects and does not imply universal stop.
- Installation safe mode is part of the product architecture.
- Pause/resume new work is separate from cancelling or pausing active work.
- Drain prevents new placement while preserving in-flight tracking.
- Backup/export supports installation, account, and project scopes.
- Restore supports new/empty targets and explicit overwrite of inactive
  targets with stronger preview and confirmation.
- Exports contain canonical state plus metadata sufficient to restore or
  regenerate projections.
- Operational schedules are owned by operations; job meaning belongs to the
  target subsystem.
- Budget and quota policy supports both warnings and hard stops.
- Normalized audit export is the default; raw event export is a separate
  high-permission path.
- Stable operational mode effects are pause-new-work, drain-placement,
  pause-provider-actions, pause-client-webhooks, pause-notifications,
  read-only-public-interfaces, disable-runtime-provisioning, and
  installation-safe-mode.
- Mandatory Compose health checks cover Postgres, API, web, MCP, daemon,
  runtime worker, projection worker, provider worker, webhook worker, and
  notification worker.
- v1 backup/export artifacts use a versioned archive containing canonical
  event export, metadata, schema manifest, redaction manifest, and optional
  projection snapshots.
- Before v1, restore supports new/empty targets only unless explicitly marked
  destructive. At v1 and later, restore supports new/empty targets and
  overwrite of inactive targets with preview.
- Default retention is 30 days for successful webhook bodies, 90 days for
  failed webhook diagnostics, 90 days for runtime diagnostic output, and
  operator-configurable retention for normalized audit exports.
- Default schedules cover provider health checks, projection reconciliation,
  proof freshness scans, proactive assessment, observation digests, backup
  validation, cleanup, and quota refresh.
- Hard-stop budget policy requires provider usage data that is current enough
  for enforcement; otherwise Tanren emits warning-only policy events.
- Raw event export requires installation- or account-scope forensic export
  permission and records a durable audit event.

## Rejected Alternatives

- **Operations as shell runbooks only.** Rejected because Tanren-controlled
  state requires typed events, policy, audit, and visibility.
- **Incident mode as global cancellation.** Rejected because incidents are
  scoped and may not require stopping unrelated work.
- **Maintenance mode and incident mode as the same state.** Rejected because
  planned maintenance and reactive safety response need different semantics.
- **Restore without preview.** Rejected because restore can replace or omit
  meaningful state and must be understood before mutation.
- **Repo-local projections as backup canon.** Rejected because canonical state
  and projection metadata are sufficient to regenerate Tanren-owned assets.
- **Missing provider usage as zero cost.** Rejected because absent provider
  data should not imply no cost or quota risk.
- **Raw event export as ordinary audit export.** Rejected because raw events
  are broader than normalized operational audit and need stronger permission.
- **Scheduled jobs owning subsystem semantics.** Rejected because the scheduler
  controls when work runs; each subsystem owns what the work means.
