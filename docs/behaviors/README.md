# Tanren User Behaviors

This directory is the **what-the-user-can-do** layer of Tanren documentation.
It sits between product vision (`docs/rewrite/MOTIVATIONS.md`,
`docs/rewrite/HLD.md`) and implementation plans (`docs/rewrite/ROADMAP.md`,
`docs/rewrite/tasks/`).

A "behavior" is a high-level capability expressed in user-visible terms. It
states what a person using Tanren can accomplish, not how the system delivers
it. Behaviors are stable reference points that lane briefs, specs, and audits
can cite as acceptance targets.

## What is in this directory

| File | Purpose |
|------|---------|
| `README.md` | Authoring rules and index of all behaviors (this file) |
| `personas.md` | Canonical actor definitions referenced by every behavior |
| `interfaces.md` | Canonical interface IDs referenced by every behavior |
| `concepts.md` | Canonical domain terminology (project, spec, milestone, etc.) |
| `B-XXXX-<slug>.md` | One behavior per file, stable ID |

## Behavior file format

Each behavior file uses YAML frontmatter followed by short prose sections.

```yaml
---
id: B-0001
title: <imperative phrase, user-visible>
personas: [solo-dev, team-dev]              # IDs from personas.md
interfaces: [cli, api, mcp]                 # IDs from interfaces.md, or [any]
contexts: [personal, organizational]        # one or both
status: draft | accepted | deprecated
supersedes: []                              # behavior IDs this replaces
---
```

Body sections, in order, all short:

1. **Intent** — one sentence of the form
   *"A `<persona>` can `<verb>` so that `<outcome>`."*
2. **Preconditions** — what must already be true from the user's point of view
   (e.g. "has an active project", "has permission to act on the teammate's
   work"). Not implementation state.
3. **Observable outcomes** — what the user perceives on success. Testable,
   but expressed without reference to code shapes.
4. **Out of scope** — explicit non-goals. Prevents scope creep into how.
5. **Related** — links to other behavior IDs (`B-0007`, `B-0012`).

## Authoring rules

These are hard rules. Violations should fail review.

1. **User-visible vocabulary only.** No crate names, trait names, SQL tables,
   state-machine state names, or internal type names. Use the terms defined in
   `concepts.md`.
2. **Phrasing is capability, not specification.** Use *"the user can"* or
   *"a `<persona>` can"*. Never *"the system shall"* or *"the service MUST"*.
3. **Describe outcomes, not flows.** If a behavior needs numbered steps, it is
   too low-level. Split it, or promote the steps into a lane brief.
4. **Every behavior names at least one persona, one interface, and one
   context.** Use `any` for persona or interface only when the behavior is
   truly surface-agnostic.
5. **IDs are immutable.** Deprecate by setting `status: deprecated` and naming
   the replacement(s) in the successor's `supersedes`. Do not reuse IDs.
6. **One behavior per file.** Keep file length short. Favor splitting over
   packing.

## Cross-cutting concerns

These apply to every behavior. Do not repeat them per file unless a behavior
deviates from the default.

### Multi-project reality

A user may be active on multiple projects simultaneously. Projects can be
related or depend on each other. Behaviors should treat "project" as a scope
the user can switch between. Behaviors that only make sense within a single
project should state that in **Preconditions** ("an active project is
selected").

### Scope and permissions

Scope (own / team / cross-team) and specific permissions (e.g. "act on another
developer's in-flight work") are **preconditions**, not persona identity. A
`team-dev` or `observer` may or may not have a given scope or permission
depending on how their setup is configured. Behaviors that depend on scope or
a permission must name it explicitly in **Preconditions**. Separate meta
behaviors cover how scope and permissions are granted and revoked.

### Device reach

Every behavior should be achievable via at least one interface that works on
each supported device class — phone, low-power laptop, full laptop. Phone
access is through `api` (web or mobile client) or `mcp` (chat clients); `cli`
and `tui` are laptop-only. A behavior that genuinely cannot work on a phone
must state this in **Out of scope**.

### External issue trackers

Tanren is the system of record for specs. External tracker integration is
one-way outbound. See `concepts.md` for the details that behaviors should
assume.

## Relationship to other docs

- `docs/rewrite/MOTIVATIONS.md` — **why** Tanren exists
- `docs/behaviors/` — **what** users can do (this directory)
- `docs/rewrite/ROADMAP.md` and `docs/rewrite/tasks/` — **how** and **when**

Lane briefs should cite behavior IDs (e.g. "Completes `B-0014`, `B-0021`") so
that delivery progress maps back to user-visible capability.

## Index

<!-- Keep this list alphabetical by ID. Add each new behavior here. -->

### Project setup and switching

- [B-0025](B-0025-connect-existing-repo.md) — Connect Tanren to an existing repository
- [B-0026](B-0026-create-new-project.md) — Create a new project from scratch
- [B-0027](B-0027-see-all-projects-with-attention.md) — See all projects in an account with attention indicators
- [B-0028](B-0028-switch-active-project.md) — Switch the active project within an account
- [B-0029](B-0029-cross-project-spec-dependency.md) — Honor cross-project spec dependencies
- [B-0030](B-0030-disconnect-project.md) — Disconnect a project from Tanren
- [B-0031](B-0031-see-configure-project-access.md) — See and configure who has access to a project

### Running the implementation loop

- [B-0001](B-0001-start-loop-manually.md) — Start an implementation loop manually on a spec
- [B-0002](B-0002-auto-start-next-spec.md) — Automatically start the next spec when the current one finishes
- [B-0003](B-0003-see-loop-state.md) — See the current state of an implementation loop
- [B-0004](B-0004-notifications.md) — Be notified when a loop completes or needs attention
- [B-0005](B-0005-respond-to-blocker.md) — Respond to a question when a loop pauses on a blocker
- [B-0006](B-0006-trigger-walk-after-completion.md) — Trigger a walk to review a completed loop
- [B-0007](B-0007-pause-resume-loop.md) — Manually pause and resume an active loop
- [B-0008](B-0008-inspect-live-activity.md) — Inspect detailed live activity of a running loop
- [B-0017](B-0017-block-loop-on-unfinished-dependencies.md) — Block starting a loop on a spec with unfinished dependencies

### Team coordination

- [B-0009](B-0009-see-teammates-active-work.md) — See teammates' active work alongside my own
- [B-0010](B-0010-assist-teammate-loop.md) — Temporarily assist a teammate's in-flight loop
- [B-0011](B-0011-transfer-loop-ownership.md) — Transfer ownership of a loop to another team-dev
- [B-0012](B-0012-configure-assist-takeover-rules.md) — Configure when teammates can assist or take over each other's loops
- [B-0013](B-0013-prevent-concurrent-loops-on-spec.md) — Prevent concurrent loops on the same spec
- [B-0014](B-0014-see-action-history.md) — See the history of human actions on a loop
- [B-0015](B-0015-request-takeover.md) — Request a takeover from the current owner
- [B-0016](B-0016-see-loops-under-grouping.md) — See all active loops under a milestone or initiative

### Spec authoring and lifecycle

- [B-0018](B-0018-shape-spec-from-ticket.md) — Shape a spec from an external ticket
- [B-0019](B-0019-mark-spec-ready.md) — Mark a spec ready to run
- [B-0020](B-0020-move-spec-backlog.md) — Move a spec to or from the backlog
- [B-0021](B-0021-see-spec-state.md) — See a spec's current lifecycle state
- [B-0022](B-0022-archive-spec.md) — Archive a spec without implementation
- [B-0023](B-0023-group-specs-into-milestone.md) — Group specs into a milestone
- [B-0024](B-0024-group-milestones-into-initiative.md) — Group milestones into an initiative

### Observation and velocity

- [B-0032](B-0032-see-work-pipeline.md) — See the work pipeline — velocity and throughput
- [B-0033](B-0033-see-quality-signals.md) — See quality signals across work
- [B-0034](B-0034-see-health-signals.md) — See health signals of active work
- [B-0035](B-0035-compare-time-windows.md) — Compare metrics across time windows
- [B-0036](B-0036-see-per-developer-breakdown.md) — See a per-developer breakdown of contribution
- [B-0037](B-0037-scope-observation-views.md) — Scope observation views by project, grouping, or team

### Permissions and scope

- [B-0038](B-0038-manage-role-templates.md) — Manage roles as permission templates
- [B-0039](B-0039-see-my-permissions.md) — See my own permissions
- [B-0040](B-0040-configure-organization-policy.md) — See and configure organization-level policy
- [B-0042](B-0042-see-permission-change-history.md) — See the history of permission changes

### Configuration and credentials

- [B-0043](B-0043-create-account.md) — Create an account
- [B-0044](B-0044-invite-to-organization.md) — Invite a person to an organization
- [B-0045](B-0045-join-organization.md) — Join an organization with an existing account
- [B-0046](B-0046-switch-active-account.md) — Switch the active account
- [B-0047](B-0047-switch-active-organization.md) — Switch the active organization within an account
- [B-0048](B-0048-manage-user-configuration.md) — Manage user-tier configuration and credentials
- [B-0049](B-0049-manage-project-configuration.md) — Manage project-tier configuration
- [B-0050](B-0050-manage-organization-configuration.md) — Manage organization-tier configuration
- [B-0051](B-0051-configuration-syncs-across-devices.md) — Configuration follows the account across devices

### External tracker integration

- [B-0052](B-0052-connect-external-tracker.md) — Connect a project to an external issue tracker
- [B-0053](B-0053-see-unshaped-tickets.md) — See external tickets that are not yet shaped into specs
- [B-0054](B-0054-see-outbound-issues.md) — See outbound issues Tanren has pushed to external trackers
- [B-0055](B-0055-review-outbound-issues.md) — Review outbound issues before they are pushed
- [B-0056](B-0056-see-external-reference-state.md) — See the current state of external issues referenced by a spec
- [B-0057](B-0057-quick-access-external-links.md) — Quick-access external links from a spec to its ticket and pull request
