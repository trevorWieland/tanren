# Personas

Canonical list of product personas and external client identities referenced by
behavior files. Every behavior's `personas:` frontmatter field must use one or
more IDs from this document.

Personas describe **how an actor relates to Tanren work**, not their job title,
device, permission grants, or technical skill. Scope (own / project /
organization / account, as defined in `concepts.md`), permissions, and device
class are orthogonal dimensions captured as preconditions on individual
behaviors.

One human may occupy more than one persona depending on project and scope. A
solo founder may be a `solo-builder`, `operator`, and `observer` at the same
time. A platform engineer may be an `operator` for one organization and a
`team-builder` on a project they actively shape.

Technical depth is not a persona. A technical and non-technical builder should
be able to drive the same product-planning, shaping, walking, and acceptance
flows. Their language, guidance needs, and desire to inspect code or runtime
runtime details may differ, but the durable behavior contract is the same.

The strategic builder archetype for Tanren is a technical product builder: a
person who thinks in long-term vision, user problems, and product outcomes,
while having enough technical judgment to shape real solutions. This archetype
is represented through `solo-builder` or `team-builder` depending on whether
the work is individual or shared; it is not a separate persona ID.

Tanren's strategic default is team building. Solo building is still first-class
because it is the smallest useful case of the same method: product planning,
behavior identification, roadmap sequencing, spec execution, behavior proof,
source references, and walks should scale from one builder to a governed team.

To add a new persona, add a section below with a stable ID slug, a one-sentence
definition, and a short list of what they care about. Do not rename IDs once
they are in use after behavior canon is locked. System/runtime components are
not personas; define them in the runtime and related subsystem architecture
records.

---

## `solo-builder`

The only person actively driving product or implementation work for a project
through Tanren. No other Tanren user's activity touches the same project.
Covers:

- A founder building a product alone
- A product manager creating and walking work for a prototype
- A developer or maintainer working alone on a repository
- A Tanren maintainer working on Tanren itself
- A technical product builder using Tanren's team method alone

Cares about:

- Low setup friction and clear guidance
- Being able to plan, shape, run, walk, and accept work without coordinating
  with other users
- Owning their own credentials and project direction
- Avoiding forced collaboration or governance concepts they do not need

## `team-builder`

One of several people actively driving shared product or implementation work
through Tanren. Their activity can intersect with teammates' work. Covers:

- A developer on a team at a company
- A product manager shaping and walking work with a technical team
- A technical product builder coordinating agent-assisted delivery
- A technical lead coordinating implementation across multiple specs
- A group of maintainers working on a shared project or OSS repository

Cares about:

- Shared, reviewable product and project context
- Per-user credentials that do not leak across teammates
- Clear ownership, handoff, assist, review, and approval behavior
- Repeatable workflows that live alongside the project

## `observer`

A person who watches Tanren progress, proof status, health, risk, or outcomes but
does not normally intervene in execution or configuration. Coverage and action
scope depend on configured permissions. Covers:

- A CTO, director, engineering manager, or program lead tracking progress
- A stakeholder watching whether product work is moving safely
- A security, compliance, or quality reviewer with read-only audit visibility

Cares about:

- Read-only visibility at the scope they are granted
- Velocity, throughput, quality, health, and risk signals
- Source references showing that work is aligned with product and governance expectations
- Not being asked to configure or operate Tanren to get value

## `operator`

A person responsible for keeping Tanren usable, governed, secure, and healthy
for a project, organization, or installation. Operators may or may not be
developers. Covers:

- A solo self-hosted user responsible for their own Tanren installation
- A platform or DevOps owner managing workers, queues, and execution targets
- A security or governance owner managing policy, credentials, and approvals
- An administrator responsible for upgrades, recovery, and operational safety

Cares about:

- Worker, queue, daemon, and execution-target health
- Safe credential, secret, policy, and permission management
- The ability to pause, resume, drain, recover, audit, upgrade, and restore
  Tanren-controlled work
- Clear source references for operational and governance decisions

## `integration-client`

An external system, script, webhook, CI job, or automation that uses Tanren's
public contracts to create, update, or observe Tanren state. Covers:

- Source-control or CI integrations reporting review and build state
- External tracker integrations contributing intake or outbound issue state
- Organization automation provisioning projects, accounts, or configuration
- Scripts using Tanren's API, CLI, or MCP surface as a stable contract

Cares about:

- Stable machine-readable contracts
- Idempotent, attributable state changes
- Clear validation errors and permission boundaries
- No dependency on internal crate, table, or struct shapes
