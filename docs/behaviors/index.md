# Behavior Catalog

This directory is the **what-the-user-can-do** layer of Tanren documentation.
It sits between product intent and executable work:

```text
product vision -> behavior catalog -> architecture -> implementation assessment -> roadmap DAG -> shaped specs -> behavior proof
```

A "behavior" is a high-level capability expressed in user-visible terms. It
states what a person using Tanren can accomplish, not how the system delivers
it. Behaviors are stable reference points that lane briefs, specs, and audits
can cite as acceptance targets.

Behaviors are the product contract between planning and proof. A roadmap node
is only executable if it completes at least one accepted behavior. A spec demo
is meaningful because it shows the accepted behaviors that now exist. A BDD
scenario is valuable because it asserts a behavior rather than an incidental
implementation detail.

Behavior files are portable product contracts. Behavior proof, implementation
source references, roadmap nodes, specs, verification reports, and code should
cite behavior IDs; behavior files should not cite implementation-specific
artifacts.

## What is in this directory

| File | Purpose |
|------|---------|
| `index.md` | Authoring rules and index of all behaviors (this file) |
| `B-XXXX-<slug>.md` | One behavior per file, stable ID |

Supporting owned projections:

- `docs/product/personas.md` defines persona IDs.
- `docs/product/concepts.md` defines product concepts and scopes.
- Runtime actor IDs are defined by runtime and related subsystem architecture
  records.
- `docs/experience/surfaces.yml` defines active project surface IDs.
- `docs/architecture/subsystems/interfaces.md` defines Tanren's own public
  surfaces during the `interfaces:` to `surfaces:` migration.
- `docs/implementation/verification.md` will summarize current verification
  state once any implementation exists. It is produced by the
  `assess-implementation` skill and is absent pre-Foundation; Tanren has no
  implementation to assess until F-0001 lands.

## Behavior file format

Each behavior file uses YAML frontmatter followed by short prose sections.

```yaml
---
schema: tanren.behavior.v0
id: B-0001
title: <imperative phrase, user-visible>
area: implementation-loop                  # stable product area slug
personas: [solo-builder, team-builder]      # IDs from docs/product/personas.md
runtime_actors: []                          # optional IDs from architecture
surfaces: [web, api, mcp, cli, tui]         # IDs from docs/experience/surfaces.yml
contexts: [personal, organizational]        # one or both
product_status: draft | accepted | deprecated | removed
verification_status: unimplemented | implemented | asserted | retired
supersedes: []                              # behavior IDs this replaces
---
```

`product_status` and `verification_status` track different facts:

- `product_status` says whether the behavior is part of Tanren's product canon.
- `verification_status` says whether working code and executable behavior proof exist.

A behavior may be product-accepted but not implemented, or product-deprecated
while still asserted by compatibility tests. Do not collapse these concepts into
one field.

Product status values:

- `draft` — proposed behavior, not yet accepted as product canon.
- `accepted` — canonical behavior Tanren intends to support.
- `deprecated` — historical or transitional behavior that should not guide new
  work.
- `removed` — retired behavior ID kept only as a tombstone for traceability.

Verification status values:

- `unimplemented` — no accepted code path exists.
- `implemented` — code appears to support the behavior, but active behavior proof
  is missing.
- `asserted` — active executable behavior proof exists.
- `retired` — no active implementation or assertion is expected.

`asserted` always requires active BDD coverage with both a positive witness and
a falsification witness. Exceptions require an explicit note in the behavior
file and should be rare.

Body sections, in order, all short:

1. **Intent** — one sentence of the form
   *"A `<persona>` can `<verb>` so that `<outcome>`."* Runtime actor behaviors
   may use "a runtime actor" or the actor ID when the behavior is a protocol
   contract rather than a user-facing action.
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
   `docs/product/concepts.md`.
2. **Phrasing is capability, not specification.** Use *"the user can"* or
   *"a `<persona>` can"*. Never *"the system shall"* or *"the service MUST"*.
3. **Describe outcomes, not flows.** If a behavior needs numbered steps, it is
   too low-level. Split it, or promote the steps into a lane brief.
4. **Every behavior names at least one persona, one surface, and one
   context.** Do not use `any` for personas; list the specific product personas
   or external clients that care about the behavior. `runtime_actors` may be
   added only for internal runtime subjects defined in runtime and related
   subsystem architecture records.

   The `surfaces` field MUST be a subset of `docs/experience/surfaces.yml`.
   Existing Tanren behavior files still use `interfaces:`; validators treat
   that field as a compatibility alias until the catalog migrates. The legacy
   `any` marker is forbidden, as is `daemon` (an internal actor, not a public
   surface). The list represents the architectural commitment of where this
   behavior is reachable to its declared personas — not a description of how
   it is implemented. Adding or removing a surface is a behavior change.

   Default for Tanren human-facing behaviors (any persona in
   `{solo-builder, team-builder, observer, operator}`):
   `[web, api, mcp, cli, tui]`. Narrower lists require a clear product reason
   stated in the behavior body or the `Out of scope` section. Common
   exceptions:

   - Machine-only contracts (only persona is `integration-client`):
     `[api, mcp]`.
   - Repository-bootstrap behaviors that necessarily run before the
     repository's web/tui surfaces are reachable: `[cli]`.
5. **Every behavior declares one `area`.** Areas are stable roadmap groupings,
   not implementation modules. Examples: `project-setup`,
   `implementation-loop`, `runtime-substrate`, `planner-orchestration`,
   `governance`, `configuration`, `external-tracker`, `observation`,
   `integration-management`, `integration-contract`, `runtime-actor-contract`,
   `product-discovery`, `architecture-planning`, `implementation-assessment`,
   `behavior-proof`, `proactive-analysis`, `spec-quality`.
6. **IDs are immutable once accepted or asserted.** Draft IDs may be reorganized
   during catalog-polish work, but accepted or asserted IDs must be deprecated
   or removed rather than silently repurposed. Name replacements in the
   successor's `supersedes`.
7. **One behavior per file.** Keep file length short. Favor splitting over
   packing.
8. **One behavior ID per BDD scenario.** A `.feature` file should normally cover
   one behavior. If that becomes awkward, split the behavior before grouping
   unrelated acceptance targets together.
9. **Behaviors must survive refactors.** They describe durable user outcomes
   for people, API clients, CLI users, administrators, agents, or workers. They
   must not depend on crate names, table shapes, internal structs, or current
   code organization.
10. **Proof and source links point toward behavior IDs.** Keep behavior files portable.
    Repository-local proof, source references, and implementation artifacts
    should cite behavior IDs rather than being cited from behavior frontmatter.

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

Scope (own / project / organization / account, as defined in
`docs/product/concepts.md`) and specific permissions (e.g. "act on another user's in-flight
work") are **preconditions**, not persona identity. A
`team-builder` or `observer` may or may not have a given scope or permission
depending on how their setup is configured. Behaviors that depend on scope or
a permission must name it explicitly in **Preconditions**. Separate meta
behaviors cover how scope and permissions are granted and revoked.

Personas are not authorization roles. A persona describes why an actor uses
Tanren; a permission describes what they may do. Behavior docs must not encode
rules such as "operators can" or "observers cannot" unless the behavior is
about that relationship to work. Authorization-oriented outcomes should name
permissions and policy boundaries, not persona IDs. Roles are arbitrary,
user-defined collections of permissions and should remain a governance concept
rather than a fixed code-level enum.

Runtime actors are not personas. They describe Tanren-controlled components
whose durable protocol obligations are user-relevant, such as an `agent-worker`
honoring scoped access or reporting progress. Runtime actors belong in
`runtime_actors`, not in `personas`.

### Device reach

Every behavior should be achievable via at least one surface that works on each
supported device class for the adopting project. Tanren's `web` surface is
responsive and works on phone and laptop. `mcp` is reachable from phone chat
clients. `api` is reachable from any client (web, mobile native, or external
automation). `cli` and `tui` are laptop-only. A behavior that genuinely cannot
work on a supported device class must state this in **Out of scope**.

### External issue trackers

Tanren is the system of record for specs. External tracker integration is
one-way outbound. See `docs/product/concepts.md` for the details that behaviors
should assume.

## Relationship to other planning layers

- Product vision — **why** the product exists, who it serves, and what outcomes
  matter.
- `docs/behaviors/` — **what** users, operators, clients, or runtime actors
  can do.
- Architecture — **how** the product is intended to be implemented.
- Implementation assessment — **what** appears true in the current repo.
- Roadmap DAG — **when** and in what dependency order spec-sized behavior
  increments should be built.
- Shaped specs — **how** one roadmap node becomes acceptance criteria, demo
  steps, tasks, and proof obligations.
- BDD features — **whether** accepted behaviors are asserted through positive
  and falsification witnesses.

Every executable roadmap node must complete at least one accepted behavior.
Every asserted behavior must have active behavior proof with both a positive
witness and a falsification witness.

## Index

<!-- Keep this list grouped by area and sorted by ID within each area. -->

### Project Setup

- [B-0025](B-0025-connect-existing-repo.md) — Connect Tanren to an existing repository
- [B-0026](B-0026-create-new-project.md) — Create a new project from scratch
- [B-0027](B-0027-see-all-projects-with-attention.md) — See all projects in an account with attention indicators
- [B-0028](B-0028-switch-active-project.md) — Switch the active project within an account
- [B-0030](B-0030-disconnect-project.md) — Disconnect a project from Tanren
- [B-0068](B-0068-bootstrap-tanren-assets.md) — Bootstrap Tanren assets into an existing repository
- [B-0069](B-0069-detect-installer-drift.md) — Detect installer drift without mutating files
- [B-0070](B-0070-generate-selected-agent-integrations.md) — Generate selected agent integrations deterministically
- [B-0071](B-0071-load-runtime-standards-root.md) — Use the repository's installed standards
- [B-0134](B-0134-upgrade-installed-tanren-assets.md) — Upgrade installed Tanren assets
- [B-0135](B-0135-uninstall-tanren-assets-without-deleting-user-work.md) — Uninstall Tanren assets without deleting user work
- [B-0136](B-0136-complete-first-run-setup.md) — Complete first-run setup
- [B-0137](B-0137-choose-deployment-posture.md) — Choose a deployment posture
- [B-0138](B-0138-connect-identity-source-control-provider.md) — Select first-run identity and source-control providers
- [B-0139](B-0139-reach-ready-first-project.md) — Reach a ready first project
- [B-0186](B-0186-join-existing-project-with-context.md) — Join an existing project with current context

### Product Discovery

- [B-0140](B-0140-turn-vague-idea-into-project-brief.md) — Turn a vague idea into a project brief
- [B-0141](B-0141-define-target-users-and-problems.md) — Define target users and problems
- [B-0142](B-0142-define-product-non-goals-and-constraints.md) — Define product non-goals and constraints
- [B-0143](B-0143-define-product-success-signals.md) — Define product success signals
- [B-0144](B-0144-import-planning-context.md) — Import existing planning context

### Product Planning

- [B-0079](B-0079-bootstrap-product-mission.md) — Bootstrap a product mission for a project
- [B-0092](B-0092-create-update-product-roadmap.md) — Maintain a product roadmap as planning context
- [B-0098](B-0098-keep-work-aligned-with-product-mission.md) — Keep work aligned with product mission
- [B-0189](B-0189-propose-planning-change.md) — Propose a planning change without accepting it
- [B-0190](B-0190-review-proposed-planning-change.md) — Review a proposed planning change
- [B-0276](B-0276-maintain-accepted-behavior-catalog.md) — Maintain the accepted behavior catalog
- [B-0277](B-0277-see-behavior-coverage-verification-status.md) — See behavior coverage and verification status
- [B-0288](B-0288-review-behavior-catalog-coherence.md) — Review behavior catalog coherence

### Architecture Planning

- [B-0281](B-0281-maintain-system-architecture-record.md) — Maintain the system architecture record
- [B-0282](B-0282-review-architecture-tradeoffs.md) — Review architecture tradeoffs

### Implementation Assessment

- [B-0283](B-0283-assess-implementation-against-behaviors.md) — Assess implementation against accepted behaviors
- [B-0284](B-0284-review-implementation-assessment-uncertainty.md) — Review implementation assessment uncertainty

### Prioritization

- [B-0158](B-0158-compare-candidate-work.md) — Compare candidate work before prioritization
- [B-0159](B-0159-recommend-roadmap-sequencing.md) — Recommend roadmap sequencing with tradeoffs
- [B-0160](B-0160-defer-work-with-rationale.md) — Defer work with an explicit rationale
- [B-0161](B-0161-rebalance-roadmap-after-new-source-signals.md) — Rebalance the roadmap after new source signals

### Intake

- [B-0018](B-0018-shape-spec-from-ticket.md) — Create a draft spec manually
- [B-0075](B-0075-prefill-draft-spec-from-external-ticket.md) — Prefill a draft spec from an external ticket
- [B-0093](B-0093-turn-roadmap-items-into-specs.md) — Turn roadmap items into specs
- [B-0094](B-0094-ingest-customer-feedback.md) — Capture human-authored product signals as candidate work
- [B-0097](B-0097-turn-audit-findings-into-work.md) — Turn audit findings into specs or backlog items
- [B-0278](B-0278-classify-bug-reports-against-behavior-status.md) — Classify bug reports against behavior status

### Repo Understanding

- [B-0145](B-0145-analyze-imported-repository.md) — Analyze an imported repository before planning work
- [B-0146](B-0146-detect-project-commands.md) — Detect build, test, lint, and release commands
- [B-0147](B-0147-summarize-architecture-and-risks.md) — Summarize architecture and major risk areas
- [B-0148](B-0148-propose-initial-project-configuration.md) — Review initial project configuration proposals from repo source signals

### Standards Evolution

- [B-0149](B-0149-discover-project-standards-from-source-signals.md) — Discover project standards from repo source signals
- [B-0150](B-0150-propose-standards-updates-from-findings.md) — Propose standards updates from repeated findings
- [B-0151](B-0151-explain-standards-influence.md) — Explain which standards influenced a decision
- [B-0152](B-0152-detect-conflicting-standards.md) — Detect conflicting standards before work starts

### Spec Lifecycle

- [B-0019](B-0019-mark-spec-ready.md) — Mark a spec ready to run
- [B-0020](B-0020-move-spec-backlog.md) — Move a spec to or from the backlog
- [B-0021](B-0021-see-spec-state.md) — See a spec's current lifecycle state
- [B-0022](B-0022-archive-spec.md) — Archive a spec without implementation
- [B-0023](B-0023-group-specs-into-milestone.md) — Group specs into a milestone
- [B-0024](B-0024-group-milestones-into-initiative.md) — Group milestones into an initiative
- [B-0029](B-0029-cross-project-spec-dependency.md) — Honor cross-project spec dependencies
- [B-0061](B-0061-bulk-actions-on-specs.md) — Perform bulk actions on multiple specs
- [B-0076](B-0076-define-acceptance-criteria-for-spec.md) — Define acceptance criteria for a spec
- [B-0077](B-0077-declare-dependencies-for-spec.md) — Declare dependencies for a spec
- [B-0078](B-0078-shape-draft-spec-for-prioritization.md) — Shape a draft spec for prioritization

### Spec Quality

- [B-0153](B-0153-ask-clarifying-questions-for-vague-work.md) — Ask clarifying questions for vague work
- [B-0154](B-0154-identify-missing-edge-cases.md) — Identify missing edge cases in a spec
- [B-0155](B-0155-detect-oversized-specs.md) — Detect oversized specs and propose splits
- [B-0156](B-0156-block-untestable-spec-readiness.md) — Block readiness for untestable behavior
- [B-0157](B-0157-explain-why-spec-not-ready.md) — Explain why a spec is not ready

### Implementation Loop

- [B-0001](B-0001-start-loop-manually.md) — Start an implementation loop manually on a spec
- [B-0002](B-0002-auto-start-next-spec.md) — Automatically start eligible ready work when serial execution is configured
- [B-0003](B-0003-see-loop-state.md) — See the current state of an implementation loop
- [B-0004](B-0004-notifications.md) — Be notified when a loop completes or needs attention
- [B-0005](B-0005-respond-to-blocker.md) — Respond to a question when a loop pauses on a blocker
- [B-0006](B-0006-trigger-walk-after-completion.md) — Start a walk for implementation-ready work
- [B-0007](B-0007-pause-resume-loop.md) — Manually pause and resume an active loop
- [B-0008](B-0008-inspect-live-activity.md) — Inspect detailed live activity of a running loop
- [B-0017](B-0017-block-loop-on-unfinished-dependencies.md) — Block starting a loop when declared dependencies are not usable
- [B-0058](B-0058-cancel-loop.md) — Cancel a loop
- [B-0265](B-0265-coordinate-candidate-implementations.md) — Coordinate candidate implementations for one spec

### Runtime Substrate

- [B-0099](B-0099-add-code-harness-credentials.md) — Add credentials for a code harness
- [B-0100](B-0100-choose-project-harnesses.md) — Choose which harnesses a project may use
- [B-0101](B-0101-see-harness-readiness.md) — See whether a harness is ready to run work
- [B-0102](B-0102-start-work-isolated-environment.md) — Start work in an isolated execution environment
- [B-0103](B-0103-see-where-active-work-runs.md) — See where active work is running
- [B-0104](B-0104-see-execution-environment-access.md) — See what access an execution environment has
- [B-0105](B-0105-stop-or-recover-interrupted-execution.md) — Stop or recover interrupted execution
- [B-0106](B-0106-see-harness-neutral-runtime-failure.md) — See runtime failure source signals in a harness-neutral form
- [B-0107](B-0107-retry-transient-runtime-failures.md) — Retry transient runtime failures without duplicating visible work
- [B-0108](B-0108-manage-vm-remote-execution-targets.md) — Manage VM or remote execution targets
- [B-0109](B-0109-retire-or-drain-execution-target.md) — Retire or drain an execution target
- [B-0234](B-0234-grant-worker-scoped-temporary-access.md) — Grant worker-scoped temporary access
- [B-0235](B-0235-revoke-active-worker-access.md) — Revoke active worker access
- [B-0241](B-0241-manage-cloud-vm-provider-accounts.md) — Manage cloud or VM provider accounts

### Runtime Actor Contracts

- [B-0243](B-0243-receive-scoped-worker-assignment.md) — Receive a scoped worker assignment
- [B-0244](B-0244-refuse-invalid-worker-assignment.md) — Refuse invalid or unauthorized worker assignments
- [B-0245](B-0245-report-worker-progress.md) — Report worker progress durably
- [B-0246](B-0246-submit-worker-result-artifacts.md) — Submit worker result artifacts for assigned work
- [B-0247](B-0247-report-worker-blockers.md) — Report blockers with actionable options
- [B-0248](B-0248-report-worker-terminal-outcome.md) — Report terminal worker outcomes
- [B-0249](B-0249-honor-worker-cancellation.md) — Honor cancellation and access revocation
- [B-0250](B-0250-avoid-duplicate-visible-work-on-retry.md) — Avoid duplicate visible work on retry
- [B-0251](B-0251-use-only-granted-worker-access.md) — Use only granted credentials and environment access
- [B-0252](B-0252-preserve-worker-output-without-leaking-secrets.md) — Preserve worker output without leaking secrets
- [B-0253](B-0253-distinguish-worker-harness-runtime-failure.md) — Distinguish worker, harness, provider, and runtime failures
- [B-0254](B-0254-reconcile-interrupted-worker-session.md) — Resume or reconcile interrupted worker sessions

### Autonomy Controls

- [B-0162](B-0162-configure-autonomy-level.md) — Configure Tanren's autonomy level
- [B-0163](B-0163-require-approval-before-sensitive-actions.md) — Require approval before sensitive actions
- [B-0164](B-0164-let-low-risk-work-continue-until-blocker.md) — Let low-risk work continue until a blocker
- [B-0165](B-0165-stop-automation-at-boundaries.md) — Stop automation when user-set boundaries are crossed

### Planner Orchestration

- [B-0110](B-0110-see-planned-graph-of-work.md) — See the planned graph of work
- [B-0111](B-0111-see-why-work-is-blocked.md) — See why work is blocked by another graph node
- [B-0112](B-0112-see-why-next-work-chosen.md) — See why Tanren chose the next available work
- [B-0113](B-0113-see-replan-changes.md) — See when Tanren replans and what changed
- [B-0114](B-0114-compare-graph-revisions.md) — Compare graph revisions
- [B-0115](B-0115-approve-generated-plan-when-required.md) — Approve a generated plan when policy requires approval
- [B-0116](B-0116-link-proof-source-signals-to-graph-nodes.md) — Link proof and source signals to graph nodes
- [B-0266](B-0266-run-stacked-diff-dependent-spec.md) — Run a stacked-diff dependent spec against an available base
- [B-0280](B-0280-see-roadmap-behavior-coverage.md) — See roadmap behavior coverage

### Review, Merge, And Cleanup

- [B-0072](B-0072-review-delivered-behavior-during-walk.md) — Review delivered behavior during a walk
- [B-0073](B-0073-accept-walked-work.md) — Accept walked work
- [B-0074](B-0074-reject-walked-work-route-follow-up.md) — Reject walked work and route follow-up work
- [B-0117](B-0117-create-pr-from-walked-work.md) — Create a pull request from walked work
- [B-0118](B-0118-see-pr-ci-state-from-spec.md) — See pull request and CI state from the spec
- [B-0119](B-0119-route-review-feedback-follow-up-work.md) — Route review feedback into follow-up work
- [B-0120](B-0120-mark-review-feedback-addressed-out-of-scope.md) — Mark review feedback as addressed or out of scope
- [B-0121](B-0121-merge-accepted-work.md) — Merge accepted work
- [B-0122](B-0122-clean-up-completed-execution-resources.md) — Clean up completed execution resources
- [B-0123](B-0123-recover-from-merge-conflicts.md) — Recover from merge conflicts after parallel work lands
- [B-0124](B-0124-detect-intent-conflicts.md) — Detect intent conflicts after related work lands

### Walk Acceptance

- [B-0166](B-0166-present-walk-demo-summary.md) — Present a walk or demo summary before acceptance
- [B-0167](B-0167-show-expected-vs-actual-behavior.md) — Show expected versus actual behavior
- [B-0168](B-0168-show-changed-surfaces-and-residual-risks.md) — Show changed surfaces and residual risks
- [B-0169](B-0169-preserve-walk-acceptance-record.md) — Preserve the walk acceptance record

### Behavior Proof

- [B-0285](B-0285-produce-executable-behavior-proof.md) — Produce executable behavior proof

### Decision Memory

- [B-0170](B-0170-record-product-decisions-and-assumptions.md) — Record product decisions and assumptions
- [B-0171](B-0171-preserve-rejected-alternatives.md) — Preserve rejected alternatives
- [B-0172](B-0172-reuse-prior-user-preferences.md) — Reuse prior user preferences when shaping work
- [B-0173](B-0173-surface-stale-assumptions.md) — Surface stale assumptions when context changes
- [B-0191](B-0191-resolve-conflicting-product-direction.md) — Resolve conflicting product direction

### Undo And Recovery

- [B-0174](B-0174-restore-previous-revision.md) — Restore a previous revision without deleting history
- [B-0175](B-0175-revert-landed-work-through-follow-up.md) — Revert landed work through controlled follow-up
- [B-0177](B-0177-recover-from-bad-planning-decision.md) — Recover from a bad planning decision without losing history

### Release And Learning

- [B-0178](B-0178-prepare-release-notes-from-accepted-work.md) — Prepare release notes from accepted work
- [B-0179](B-0179-verify-post-release-state.md) — Verify post-release state
- [B-0180](B-0180-link-shipped-changes-to-roadmap-and-specs.md) — Link shipped changes back to roadmap and specs
- [B-0181](B-0181-ingest-post-ship-feedback.md) — Ingest post-ship feedback
- [B-0182](B-0182-update-roadmap-from-post-ship-outcomes.md) — Update roadmap and specs from post-ship outcomes
- [B-0270](B-0270-configure-shipped-definition.md) — Configure what counts as shipped for a project

### Findings

- [B-0080](B-0080-track-finding-lifecycle.md) — See unresolved check findings that block readiness

### Team Coordination

- [B-0009](B-0009-see-teammates-active-work.md) — See teammates' active work alongside my own
- [B-0010](B-0010-assist-teammate-loop.md) — Temporarily assist a teammate's in-flight loop
- [B-0011](B-0011-transfer-loop-ownership.md) — Transfer ownership of a loop to another team-builder
- [B-0012](B-0012-configure-assist-takeover-rules.md) — Configure when teammates can assist or take over each other's loops
- [B-0013](B-0013-prevent-concurrent-loops-on-spec.md) — Prevent uncoordinated concurrent loops on the same spec
- [B-0014](B-0014-see-action-history.md) — See the history of human actions on a loop
- [B-0015](B-0015-request-takeover.md) — Request a takeover from the current owner
- [B-0016](B-0016-see-loops-under-grouping.md) — See all active loops under a milestone or initiative
- [B-0187](B-0187-see-work-needing-my-attention.md) — See shared work that needs my attention
- [B-0192](B-0192-claim-work-or-review-ownership.md) — Claim ownership of work or review
- [B-0193](B-0193-assign-work-or-review.md) — Assign work or review to another builder
- [B-0194](B-0194-leave-handoff-notes.md) — Leave handoff notes for another builder
- [B-0195](B-0195-route-blocker-to-responder.md) — Route a blocker to an appropriate responder
- [B-0196](B-0196-detect-overlapping-work.md) — Detect duplicate or overlapping work across builders
- [B-0200](B-0200-onboard-builder-to-shared-project.md) — Onboard a builder to shared project context
- [B-0201](B-0201-see-team-attention-load.md) — See team attention load without performance surveillance

### Governance

- [B-0031](B-0031-see-project-access.md) — See who has access to a project
- [B-0038](B-0038-manage-role-templates.md) — Manage roles as permission templates
- [B-0039](B-0039-see-my-permissions.md) — See my own permissions
- [B-0040](B-0040-configure-organization-policy.md) — Configure organization approval policy
- [B-0042](B-0042-see-permission-change-history.md) — See the change history for a project or organization
- [B-0043](B-0043-create-account.md) — Create an account
- [B-0044](B-0044-invite-to-organization.md) — Invite a person to an organization
- [B-0045](B-0045-join-organization.md) — Join an organization with an existing account
- [B-0046](B-0046-switch-active-account.md) — Switch the active account
- [B-0047](B-0047-switch-active-organization.md) — Switch the active organization within an account
- [B-0059](B-0059-leave-organization.md) — Leave an organization
- [B-0060](B-0060-remove-member-from-organization.md) — Remove a member from an organization
- [B-0065](B-0065-see-organization-member-access.md) — See existing members' access to an organization
- [B-0066](B-0066-create-organization.md) — Create an organization
- [B-0067](B-0067-delete-organization.md) — Delete an organization
- [B-0081](B-0081-configure-runtime-placement-policy.md) — Configure organization runtime placement policy
- [B-0082](B-0082-configure-harness-allowlist-policy.md) — Configure organization harness allowlist policy
- [B-0083](B-0083-configure-budget-quota-policy.md) — Configure organization budget and quota policy
- [B-0084](B-0084-configure-organization-standards-policy.md) — Configure organization standards policy
- [B-0085](B-0085-configure-project-policy-inheritance.md) — Configure project policy inheritance and overrides
- [B-0197](B-0197-see-gated-action-approval-state.md) — See approval state for a gated action
- [B-0198](B-0198-collect-required-approvals.md) — Collect required approvals for a gated action
- [B-0202](B-0202-manage-project-access.md) — Manage project access
- [B-0203](B-0203-manage-organization-member-access.md) — Manage organization member access
- [B-0220](B-0220-create-manage-service-accounts.md) — Create and manage service accounts
- [B-0221](B-0221-manage-api-keys.md) — Create and scope API keys
- [B-0233](B-0233-configure-credential-use-policy.md) — Configure credential use policy by scope
- [B-0238](B-0238-test-policy-changes-before-applying.md) — Test policy changes before applying them
- [B-0239](B-0239-explain-policy-denial.md) — Explain why policy denied an operation
- [B-0267](B-0267-manage-organization-standards-profiles.md) — Manage organization standards profiles
- [B-0268](B-0268-apply-organization-standards-profile.md) — Apply organization standards profiles to projects
- [B-0272](B-0272-choose-project-disposition-for-organization-deletion.md) — Choose project disposition when deleting an organization
- [B-0274](B-0274-rotate-revoke-api-keys.md) — Rotate or revoke API keys

### Configuration And Credentials

- [B-0048](B-0048-manage-user-configuration.md) — Manage user-tier configuration and credentials
- [B-0049](B-0049-manage-project-configuration.md) — Manage project methodology settings
- [B-0050](B-0050-manage-organization-configuration.md) — Manage shared configuration defaults across projects
- [B-0051](B-0051-configuration-syncs-across-devices.md) — Configuration follows the account across devices
- [B-0062](B-0062-configure-notification-preferences.md) — Configure notification preferences and routing
- [B-0086](B-0086-manage-project-runtime-defaults.md) — Manage project runtime defaults
- [B-0087](B-0087-manage-project-verification-gates.md) — Manage project verification gates
- [B-0088](B-0088-manage-project-scoped-secrets.md) — Manage project-scoped secrets
- [B-0089](B-0089-see-project-configuration-change-history.md) — See project configuration change history
- [B-0125](B-0125-store-user-credentials-without-exposure.md) — Store user credentials without exposing secret values
- [B-0126](B-0126-store-shared-secrets-without-exposure.md) — Store project or organization secrets without exposing secret values
- [B-0127](B-0127-see-secret-usage-without-revealing-values.md) — See where credentials or secrets are used without revealing them
- [B-0128](B-0128-rotate-credential-or-secret.md) — Rotate a credential or secret
- [B-0129](B-0129-revoke-credential-or-secret.md) — Revoke a credential or secret
- [B-0199](B-0199-distinguish-personal-and-shared-credentials.md) — Distinguish personal credentials from shared secrets
- [B-0222](B-0222-see-api-key-service-account-usage.md) — See API key and service account usage
- [B-0231](B-0231-detect-overscoped-credentials.md) — Detect over-scoped credentials or integrations
- [B-0232](B-0232-detect-stale-credentials.md) — Detect expiring, unused, or stale credentials
- [B-0269](B-0269-configure-account-shared-project-defaults.md) — Configure shared project defaults for an account

### Integration Management

- [B-0223](B-0223-connect-organization-provider-integration.md) — Connect an organization-owned provider integration
- [B-0224](B-0224-connect-user-provider-integration.md) — Connect a user-owned provider integration
- [B-0225](B-0225-distinguish-personal-and-shared-provider-access.md) — Distinguish personal provider access from shared provider access
- [B-0226](B-0226-see-provider-connection-health.md) — See provider connection health
- [B-0227](B-0227-see-provider-permissions-reachable-resources.md) — See provider permissions and reachable resources
- [B-0228](B-0228-recover-provider-authorization-failure.md) — Recover from provider authorization failure
- [B-0229](B-0229-audit-external-integration-actions.md) — Audit external actions performed through an integration
- [B-0240](B-0240-manage-webhook-endpoints-delivery-failures.md) — Manage webhook endpoints
- [B-0275](B-0275-handle-webhook-delivery-failures.md) — Handle webhook delivery failures

### Integration Client Contracts

- [B-0255](B-0255-accept-idempotent-client-requests.md) — Accept retry-safe client create and update requests
- [B-0256](B-0256-return-machine-readable-validation-errors.md) — Return machine-readable validation errors
- [B-0257](B-0257-negotiate-api-schema-version.md) — Negotiate API and schema versions
- [B-0258](B-0258-deliver-webhooks-with-retry-ordering-dedupe.md) — Deliver webhooks reliably so event consumers can process them without ambiguity
- [B-0259](B-0259-attribute-external-automation.md) — Attribute external automation actions
- [B-0260](B-0260-report-rate-limit-backpressure.md) — Report rate limit and backpressure state
- [B-0261](B-0261-deny-machine-client-with-permission-boundary.md) — Deny machine clients across permission boundaries
- [B-0262](B-0262-observe-state-through-read-models.md) — Observe Tanren state from external systems
- [B-0263](B-0263-report-external-ci-source-status.md) — Report external CI or source-control status
- [B-0264](B-0264-replay-client-requests-after-failure.md) — Safely replay client requests after failure

### External Tracker Integration

- [B-0052](B-0052-connect-external-tracker.md) — Connect external tracker capabilities for a project
- [B-0053](B-0053-see-unshaped-tickets.md) — See external tickets that are not yet shaped into specs
- [B-0054](B-0054-see-outbound-issues.md) — See outbound issues Tanren has pushed to external trackers
- [B-0055](B-0055-review-outbound-issues.md) — Configure outbound issue review mode
- [B-0056](B-0056-see-external-reference-state.md) — See the current state of external issues referenced by a spec
- [B-0057](B-0057-quick-access-external-links.md) — Quick-access external links from a spec to its ticket and pull request
- [B-0091](B-0091-disconnect-or-replace-external-tracker.md) — Disconnect or replace an external tracker
- [B-0271](B-0271-review-queued-outbound-issues.md) — Review queued outbound issues

### Observation And Velocity

- [B-0032](B-0032-see-work-pipeline.md) — See the work pipeline — velocity and throughput
- [B-0033](B-0033-see-quality-signals.md) — See quality signals across work
- [B-0034](B-0034-see-health-signals.md) — See health signals of active work
- [B-0035](B-0035-compare-time-windows.md) — Compare metrics across time windows
- [B-0036](B-0036-see-per-developer-breakdown.md) — See a per-builder breakdown of contribution
- [B-0037](B-0037-scope-observation-views.md) — Scope observation views by project or grouping
- [B-0188](B-0188-see-what-changed-since-last-visit.md) — See what changed since I last visited the project
- [B-0204](B-0204-see-project-overview.md) — See a project overview
- [B-0205](B-0205-see-roadmap-progress-against-goals.md) — See roadmap progress against product goals
- [B-0206](B-0206-see-blocked-work-overview.md) — See what is blocked and why
- [B-0207](B-0207-see-delivery-forecast-and-risk.md) — See delivery forecast and risk
- [B-0208](B-0208-see-quality-risk-trends.md) — See quality and risk trends over time
- [B-0209](B-0209-see-source-signals-behind-status-summary.md) — See what supports a summary and how trustworthy it is
- [B-0210](B-0210-see-recently-shipped-outcomes.md) — See recently shipped outcomes
- [B-0211](B-0211-see-post-release-health-and-feedback.md) — See post-release health and feedback
- [B-0212](B-0212-see-open-decisions-and-disagreements.md) — See open decisions and unresolved disagreements
- [B-0213](B-0213-see-work-drift-from-mission-or-standards.md) — See where work drifted from mission or standards
- [B-0214](B-0214-see-cross-project-dependency-risk.md) — See cross-project dependency risk
- [B-0215](B-0215-export-read-only-status-report.md) — Export a read-only status report
- [B-0216](B-0216-subscribe-to-observer-digest.md) — Subscribe to an observer digest
- [B-0217](B-0217-compare-planned-vs-actual-delivery.md) — Compare planned versus actual delivery
- [B-0219](B-0219-see-what-changed-since-last-report.md) — See what changed since the last report

### Cross-Interface Continuity

- [B-0183](B-0183-see-coherent-state-across-interfaces.md) — See coherent Tanren state across public interfaces
- [B-0184](B-0184-continue-work-from-another-interface.md) — Continue the same work from another interface
- [B-0185](B-0185-receive-consistent-validation-errors.md) — Receive consistent validation errors across public interfaces

### Operations

- [B-0063](B-0063-export-data.md) — Export account or project data for backup or migration
- [B-0064](B-0064-restore-data.md) — Restore account or project data from a backup
- [B-0096](B-0096-run-codebase-wide-audit.md) — Run a codebase-wide audit
- [B-0130](B-0130-see-worker-queue-target-health.md) — See operational health of execution infrastructure
- [B-0131](B-0131-pause-new-work-for-scope.md) — Pause new work for a project or organization
- [B-0132](B-0132-resume-new-work-for-scope.md) — Resume new work for a project or organization
- [B-0133](B-0133-audit-placement-approval-decisions.md) — Audit placement and approval decisions
- [B-0230](B-0230-track-cost-quota-usage.md) — Track cost and quota usage across providers
- [B-0236](B-0236-enter-maintenance-or-incident-mode.md) — Enter maintenance or incident mode
- [B-0237](B-0237-run-disaster-recovery-validation.md) — Run disaster recovery validation
- [B-0242](B-0242-export-operational-audit-logs.md) — Export operational audit logs

### Proactive Analysis

- [B-0279](B-0279-route-proactive-analysis-into-planning.md) — Route proactive analysis into planning
- [B-0286](B-0286-schedule-or-run-proactive-analysis.md) — Schedule or run proactive analysis
- [B-0287](B-0287-review-proactive-analysis-recommendations.md) — Review proactive analysis recommendations
