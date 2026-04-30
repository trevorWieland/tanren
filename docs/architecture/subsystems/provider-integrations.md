---
schema: tanren.subsystem_architecture.v0
subsystem: provider-integrations
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Provider Integrations Architecture

## Purpose

This document defines Tanren's provider integration architecture. Provider
integrations are the outbound connections Tanren uses when it calls external
systems to perform or observe work.

Provider integrations make Tanren's workflow practical: source-control
providers hold repositories and pull requests, CI providers report external
checks, issue trackers exchange intake and status, infrastructure providers
create execution targets, identity providers authenticate users, notification
providers deliver messages, and external analysis providers contribute status
signals.

Provider integrations do not replace Tanren's own planning, orchestration,
runtime, identity, policy, behavior-proof, assessment, or observation
subsystems. They expose external capability through Tanren-owned contracts,
events, read models, and audit records.

## Subsystem Boundary

The provider integrations subsystem owns:

- provider connection records;
- provider capability discovery;
- provider resource mappings;
- provider authorization flows and refresh metadata;
- provider health, availability, rate-limit, and quota signals;
- provider permissions and reachable-resource summaries;
- provider action requests, retries, idempotency, and reconciliation;
- provider-side external action audit records;
- normalized inbound provider signals from polling or provider callbacks;
- bidirectional synchronization rules for provider-owned resources;
- outbound notification delivery through external channels;
- provider adapter contracts for source control, CI, issue trackers,
  infrastructure providers, identity providers, notification providers, and
  external analysis systems.

The provider integrations subsystem does not own:

- public API, MCP, CLI, TUI, or web interface contracts;
- client integrations where external systems call Tanren;
- secret value storage or encryption;
- account, organization, project, role, permission, or policy evaluation;
- runtime placement decisions;
- orchestration lifecycle decisions;
- assessment classification;
- behavior-proof semantics;
- dashboards or reports.

Client integrations are a separate subsystem. The same external platform may be
both a provider integration and a client integration, but the roles are
different: provider integrations are Tanren calling outward; client
integrations are external systems calling Tanren.

## Core Invariants

1. **Provider integrations are capability-based.** Tanren models capabilities
   such as source control, CI status, issue tracking, VM provisioning, identity
   login, notification delivery, and external analysis. A single platform
   adapter may implement many capabilities.
2. **Every project has a mergeable source-control provider.** A Tanren project
   is execution-ready only when Tanren can create, update, review, and merge
   work against the project's repository through a configured source-control
   integration.
3. **Tanren remains canonical for Tanren state.** Provider systems may mirror,
   enrich, report, or request changes, but they do not directly rewrite
   Tanren's planning, spec, orchestration, behavior-proof, or policy state
   outside accepted Tanren command pathways.
4. **External side effects are explicit actions.** Calls that mutate external
   systems are represented as durable provider actions with idempotency,
   retries, outcome recording, and audit visibility.
5. **Provider records are normalized.** Tanren stores Tanren-native provider
   state, status, references, and summaries. It does not use raw provider
   payload archives as its integration model.
6. **Connection ownership is visible.** Provider access identifies whether it
   is user-owned, project-owned, organization-owned, service-account, or
   worker-scoped.
7. **Credentials are use-only.** Provider integrations may request credential
   use through configuration and secret contracts, but secret values are never
   exposed through provider records, logs, projections, reports, or audit
   views.
8. **Execution credentials are scoped and temporary.** Execution targets may
   receive short-lived source-control credentials for assigned work, but not
   general provider administration credentials.
9. **Provider permissions are not assumed.** Tanren records discovered
   provider capability, permission, scope, and limitation metadata, and clearly
   distinguishes known, missing, ambiguous, and hidden access.
10. **Provider failure is recoverable.** Authorization failures, rate limits,
    unavailable providers, partial side effects, and stale external state must
    route to explicit recovery, retry, or reconciliation paths.

## Capability Model

Provider integrations are defined by capabilities, not by platform names:

- `source_control`: repository access, branches, commits, pull requests,
  reviews, mergeability, merge execution, and base-branch state;
- `ci_status`: external build, test, workflow, required-check, and automated
  review status;
- `issue_tracking`: external ticket intake, linked issues, bidirectional
  status and comments, and outbound issue creation;
- `execution_infrastructure`: container host, VM, remote runner, network,
  image, and target lifecycle operations;
- `identity_provider`: external login, organization identity, group claims,
  and authorization refresh mechanics;
- `notification_delivery`: outbound messages to chat, email, paging, or
  similar notification channels;
- `external_analysis`: static analysis, security scanning, dependency review,
  AI review, quality review, and other external analysis signals.

A provider adapter declares:

- provider identifier and display name;
- supported capabilities;
- supported ownership modes;
- required credential or authorization types;
- reachable-resource discovery methods;
- supported actions;
- health-check semantics;
- rate-limit and quota metadata;
- webhook or callback support where applicable;
- normalization behavior for provider statuses and errors;
- redaction requirements.

For example, a GitHub adapter may expose `source_control`,
`issue_tracking`, and `ci_status` capabilities. GitHub Actions is still modeled
as a CI capability, not as part of source control, because projects may use
GitHub for repositories while using another CI provider.

## Connection Records

A provider connection records Tanren's configured relationship with an
external provider.

Provider connection records include:

- connection identifier;
- provider identifier;
- capability set;
- owner scope: user, project, organization, service account, or worker scope;
- credential binding metadata;
- authorization status;
- expiration or refresh metadata where available;
- reachable resource summaries;
- provider permission summaries;
- health state;
- rate-limit and quota summaries where available;
- policy bindings that constrain use;
- external installation or account references;
- last successful check and last failure metadata;
- redacted diagnostic information.

Connection records do not contain secret values or unbounded provider payloads.
They may include provider resource identifiers, URLs, names, permission labels,
and redacted error summaries where those are visible under policy.

## Ownership Modes

Provider access can be owned independently from the user who configured it.

- **User-owned access** represents personal provider authorization. It is used
  when the external action must happen as, or on behalf of, a specific user.
- **Project-owned access** represents access limited to one Tanren project and
  its repository or project-specific provider resources.
- **Organization-owned access** represents shared provider access governed by
  organization policy.
- **Service-account access** represents automation credentials owned by a
  Tanren account, organization, or project, not by the human who created them.
- **Worker-scoped access** represents temporary credentials issued for one
  assignment, target, branch, provider action, or bounded time window.

Tanren must not silently substitute shared access for personal access or
personal access for shared access. If an operation requires a specific
ownership mode, missing access is reported as a provider authorization or
policy problem.

## Source Control

Every Tanren project requires a configured `source_control` capability.

The source-control integration owns provider mechanics for:

- repository discovery and binding;
- branch existence and branch protection inspection;
- Tanren-managed spec branch creation;
- fetching, cloning, and remote setup support for runtime targets;
- commit attribution support;
- push access for Tanren-managed branches;
- draft pull request creation;
- pull request metadata updates;
- ready-for-review transitions;
- pull request review status ingestion;
- mergeability and protection-rule status;
- base-branch change detection;
- merge execution after orchestration marks work merge-ready;
- reconciliation when provider state changes outside Tanren.

Orchestration owns the spec lifecycle and decides when a PR should be created,
when a candidate is ready for manual review, and when work is merge-ready.
Source-control integrations perform the external operations and report
provider reality back into Tanren.

Runtime targets may receive short-lived, branch-scoped source-control
credentials for their assigned spec branch. This allows harnesses and gates to
run repository-native workflows, commit changes, invoke hooks, and surface
immediate source-control failures inside the execution environment. The
credential scope should limit practical damage by constraining repository,
branch, permission, and lifetime.

Source-control integrations must support durable reconciliation because pull
requests, branch protection, comments, reviews, check runs, and base branches
can change outside Tanren.

## CI And External Status

CI is modeled as a separate `ci_status` capability.

CI provider integrations own:

- external workflow or pipeline status ingestion;
- commit and PR status normalization;
- required-check state;
- pending, passing, failing, cancelled, skipped, and unavailable states;
- external automated review status where exposed as checks;
- links from normalized status to external runs;
- stale or conflicting status detection;
- retry or re-run requests where the provider supports them.

CI status is not user acceptance and is not behavior proof by itself. During
orchestration, CI results participate in candidate validation alongside Tanren
internal spec checks. During assessment, CI history may become an input to
findings, trend analysis, or proactive analysis.

Providers can report CI status through outbound polling, provider callbacks,
or client integration APIs. Regardless of transport, the normalized CI record
must reference known Tanren work, source-control resources, commits, PRs,
specs, or provider resources.

## Issue And Project Trackers

Issue tracker integrations are bidirectional, but Tanren remains the system of
record for Tanren planning, specs, orchestration, and behavior state.

Issue tracker integrations own:

- importing external tickets into Tanren intake;
- linking external tickets to Tanren intake items, proposals, roadmap nodes,
  specs, tasks, findings, or releases;
- writing Tanren status summaries back to external tickets where configured;
- synchronizing comments, labels, assignees, milestones, and status mirrors
  where policy allows;
- creating outbound external issues from Tanren findings or follow-up work;
- detecting external edits that need review;
- reconciling deleted, moved, merged, or inaccessible external tickets.

Conflict rules:

- external systems may update linked fields, comments, status mirrors, intake
  metadata, and provider-owned references through configured sync;
- external systems may request Tanren planning or spec changes by creating
  intake items, findings, proposals, or comments;
- external systems may not directly rewrite accepted Tanren behavior,
  architecture, roadmap, spec, orchestration, proof, or policy state;
- conflicting provider updates are preserved as visible sync conflicts instead
  of silently overwriting Tanren state.

This keeps integrations useful for teams that live in issue trackers while
preserving Tanren's canonical planning and delivery model.

## Execution Infrastructure Providers

Execution infrastructure providers expose the external API operations needed to
create and manage runtime targets.

Infrastructure provider integrations own mechanics for:

- provider account discovery and health checks;
- image, region, machine class, network, and quota metadata;
- VM, container host, or remote runner provisioning;
- target status inspection;
- target teardown;
- provider error normalization;
- quota and cost signal collection where available;
- authorization failure recovery;
- reconciliation of leaked, missing, or drifted external targets.

Runtime owns placement decisions and target lifecycle meaning. It asks provider
integrations to perform provider operations when policy chooses a target class.
Cloud, VM, and infrastructure provider credentials stay in the Tanren control
plane. Execution targets do not receive provider administration credentials.

If an operator configures a reusable or pooled target provider, the provider
integration still treats targets as destructive sandboxes. Pooled targets must
be resettable and must not rely on persistent hand-maintained state inside the
target.

## Identity Providers

Identity provider integrations own external authentication-provider mechanics.

They may support:

- OIDC login;
- external organization or directory connection metadata;
- provider authorization refresh;
- group or claim import;
- login health checks;
- provider logout or revocation signals where available.

Identity-policy owns Tanren accounts, organizations, memberships, grants,
roles as permission templates, service accounts, and policy evaluation.
Identity provider integrations can supply external identity facts, but they do
not decide Tanren permissions by themselves.

## Notification Providers

Notification provider integrations deliver outbound messages from Tanren to
external channels.

They own:

- channel connection records;
- destination discovery where supported;
- outbound delivery actions;
- retry and backoff behavior;
- delivery failure classification;
- pause, resume, disable, and retry controls;
- redaction of message content according to recipient and channel policy;
- audit records for delivered, failed, retried, or suppressed notifications.

Notification delivery is not canonical state. A missed notification does not
erase the underlying Tanren event, task, approval request, provider failure, or
review requirement.

## External Analysis Providers

External analysis providers contribute normalized source signals from tools
outside Tanren's internal gates.

Examples include:

- external AI code review systems;
- static analysis services;
- security scanners;
- dependency scanners;
- license scanners;
- performance analysis tools;
- repository health tools.

During active orchestration, external analysis status can participate in the
candidate validation batch after draft PR creation. During assessment,
provider-reported analysis can become findings, recommendations, trend data, or
proactive analysis input.

Provider integrations normalize tool status and links. Orchestration and
assessment decide whether a result blocks work, creates tasks, creates
findings, or routes to planning.

## Provider Action Lifecycle

Every external side effect is represented as a provider action.

Provider actions include:

- action identifier;
- provider connection identifier;
- capability;
- action kind;
- Tanren actor and initiating command;
- target external resource;
- Tanren resource context;
- idempotency key;
- requested input summary;
- credential binding reference;
- policy decision reference;
- attempt count;
- provider response summary;
- normalized outcome;
- retry state;
- reconciliation state;
- audit visibility.

Provider action states:

| State | Meaning |
|---|---|
| `requested` | Tanren accepted the action request and recorded intent. |
| `authorized` | Policy and credential-use checks passed. |
| `dispatched` | A worker began the external call. |
| `succeeded` | The provider side effect completed and was normalized. |
| `failed_retryable` | The provider call failed and may be retried. |
| `failed_terminal` | The provider call failed and requires recovery or a new action. |
| `reconcile_pending` | Tanren must verify external state after ambiguity or partial success. |
| `reconciled` | External state has been inspected and Tanren knows the final outcome. |
| `cancelled` | The action was cancelled before completion where cancellation was safe. |

Retries must not duplicate external side effects when the provider supports
idempotency. When provider idempotency is unavailable or uncertain, Tanren uses
its own idempotency keys, resource mappings, and reconciliation before issuing
another mutating call.

## Provider Signals

Provider integrations emit normalized provider signals when external state is
observed through polling, callbacks, webhooks, or action results.

Provider signals can represent:

- provider connection health changes;
- authorization expiration, revocation, or denial;
- permission or reachable-resource changes;
- source-control branch, PR, review, or mergeability changes;
- CI status changes;
- issue tracker updates;
- notification delivery state;
- infrastructure target state;
- quota, cost, or rate-limit changes;
- external analysis results.

Signals are not arbitrary blobs. They are typed source signals that
other subsystems consume according to their own semantics.

## Normalization And Storage

Provider integrations store normalized Tanren records:

- provider identifiers;
- capability identifiers;
- external resource references;
- external URLs where visible;
- provider status categories;
- permission summaries;
- health summaries;
- redacted diagnostics;
- timestamps and cursors;
- action outcomes;
- sync conflict records.

Raw provider payloads are not the canonical integration model. If diagnostic
payload retention is needed for operations, it must be bounded, redacted,
access-controlled, and treated as operational support data, not as planning or
orchestration canon.

## Health, Quota, And Overscope Detection

Provider integrations expose enough metadata for users and operators to
understand provider readiness and risk.

Health states include:

- `healthy`;
- `degraded`;
- `expired`;
- `unauthorized`;
- `rate_limited`;
- `quota_limited`;
- `misconfigured`;
- `unavailable`;
- `unknown`.

Provider health checks must respect provider limits and configured polling
policy. If Tanren cannot inspect a permission, resource, quota, or cost signal,
it reports unknown or unsupported instead of implying safety.

Overscope detection compares visible provider permission metadata with intended
Tanren use. Tanren can flag broad, stale, ambiguous, or unused provider access,
but it must not claim provider permissions are safe when they cannot be
inspected.

## Security And Policy

Provider integrations depend on identity-policy and configuration-secrets.

Security requirements:

- all provider actions perform Tanren permission checks;
- all credential use follows credential-use policy;
- secret values are never persisted in provider events or read models;
- worker-scoped provider access is short-lived and least-privilege;
- provider health views respect Tanren visibility policy;
- provider resource discovery must not reveal hidden resources;
- provider diagnostics are redacted before persistence or display;
- external action audit records identify actor, provider, capability, target,
  time, and outcome;
- authorization recovery preserves connection ownership and audit history.

Service accounts are first-class actors for provider operations. A service
account can own or use provider access only within explicit grants and
credential-use policy.

## Failure And Recovery

Provider failures are normalized into stable categories:

- authorization failure;
- permission denied;
- missing resource;
- stale resource mapping;
- rate limited;
- quota exceeded;
- provider unavailable;
- timeout;
- partial success;
- ambiguous outcome;
- validation failure;
- provider contract mismatch;
- unsupported capability.

Recovery behavior depends on failure type:

- expired or revoked authorization routes to credential refresh or replacement;
- missing permission routes to provider permission correction or policy change;
- rate limits route to backoff and visible scheduling delay;
- unavailable providers route to retry and affected-work visibility;
- partial or ambiguous success routes to reconciliation before retry;
- stale resource mappings route to rediscovery or relinking;
- unsupported capability routes to configuration or adapter replacement.

Work blocked by provider failures resumes only after the connection, action,
or resource mapping is recovered.

## Events

Provider integrations emit typed events for:

- provider connection created, updated, disabled, or removed;
- provider authorization established, refreshed, expired, revoked, or denied;
- provider capability discovered or changed;
- reachable resources discovered or changed;
- provider permission summary changed;
- provider health changed;
- provider action requested, authorized, dispatched, succeeded, failed,
  cancelled, or reconciled;
- provider signal received;
- sync conflict detected or resolved;
- provider quota, cost, or rate-limit signal changed;
- external action audit record appended.

Events may record credential binding identifiers and secret metadata. They must
not record secret values.

## Read Models

Required provider integration read models include:

- provider connection list by user, project, organization, and account;
- provider connection detail with visible health, permissions, resources, and
  policy bindings;
- source-control project readiness;
- source-control PR and branch status by spec;
- CI status by commit, PR, spec, and graph node;
- external issue links and sync state;
- infrastructure provider account status and quota;
- notification delivery status;
- provider action audit;
- provider failure and recovery queue;
- overscoped, stale, or ambiguous provider access warnings.

Read models must expose freshness metadata and redaction markers so interfaces
can distinguish current, stale, hidden, unknown, and unsupported provider
state.

## Accepted Decisions

- Provider integrations and client integrations are separate architecture
  subsystems.
- Provider integrations are capability-based rather than platform-based.
- Every project requires a mergeable source-control provider.
- Source control, CI, issue tracking, execution infrastructure, identity,
  notification, and external analysis are provider capability families.
- CI is a separate capability from source control, even when the same platform
  adapter implements both.
- Issue tracker synchronization is bidirectional, but Tanren remains canonical
  for Tanren planning, spec, orchestration, proof, and policy state.
- Runtime targets may receive short-lived, branch-scoped source-control
  credentials for assigned work.
- Cloud, VM, and infrastructure administration credentials remain in the
  control plane.
- Provider side effects are represented as durable, auditable provider actions.
- Provider state is normalized into Tanren records rather than stored as raw
  provider payload canon.
- Provider capability schema includes provider ID, capability family,
  supported actions, required credential kinds, reachable resource types,
  health checks, normalized error taxonomy, rate-limit metadata, idempotency
  support, redaction behavior, and reconciliation cursors.
- GitHub is the first complete source-control provider.
- GitHub Actions is the first native CI adapter. Generic external status
  ingestion remains supported for other CI systems.
- Bidirectional issue-tracker sync covers imported issues, status mapping,
  comment/link backreferences, Tanren-created outbound issues, dedupe keys,
  and conflict markers without making external trackers canonical.
- First notification providers are email, Slack-compatible webhooks, and
  generic outbound webhook delivery.
- Provider actions run in `tanren-provider-worker`, separate from projection,
  runtime, webhook, and notification workers.
- Bounded redacted provider diagnostics are retained for 90 days by default.

## Rejected Alternatives

- **Platform-shaped integration architecture.** Rejected because one external
  platform may provide many capabilities, and one capability may be supplied by
  many platforms.
- **Source-control optional projects.** Rejected because Tanren's execution,
  branch, PR, review, and merge model requires a mergeable repository.
- **Treating GitHub Actions as part of source control.** Rejected because CI is
  a separate capability and projects may use source control from one provider
  with CI from another.
- **One-way issue tracker sync only.** Rejected because teams need external
  trackers to reflect Tanren status and comments when configured.
- **Provider raw payloads as canon.** Rejected because Tanren needs stable
  normalized contracts, redaction, policy, and replay semantics.
- **Execution targets holding cloud or VM provider credentials.** Rejected
  because provisioning and teardown are control-plane responsibilities.
- **External systems directly rewriting Tanren state.** Rejected because
  Tanren planning, orchestration, proof, and policy state must pass through
  Tanren commands, events, and policy checks.
