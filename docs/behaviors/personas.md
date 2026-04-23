# Personas

Canonical list of actor identities referenced by every behavior file. Every
behavior's `personas:` frontmatter field must use one or more IDs from this
document (or the literal `any` when truly surface-agnostic).

Personas describe **how a person relates to a project**, not their job title,
their device, or their permission grants. Scope (own / team / cross-team),
permissions (can act on others' work, can set policy), and device class
(phone, laptop) are orthogonal dimensions captured as preconditions on
individual behaviors — not as persona identity.

To add a new persona, add a section below with a stable ID slug, a one-sentence
definition, and a short list of what they care about. Do not rename IDs once
they are in use.

---

## `solo-dev`

The only Tanren user on a given project. No other Tanren user's activity
touches the same project. Covers:

- A developer on a personal side project, alone
- A solo developer at a company, on their own service or repository
- A Tanren maintainer working on Tanren itself
- An OSS maintainer working alone on their own project

Cares about:
- Low setup friction, fast local iteration
- Owning their own credentials
- Not being forced into collaboration or governance concepts they do not need

## `team-dev`

One of several Tanren users working against a shared project. Their activity
can intersect with a teammate's. What they can do on another teammate's work
(or on another team's work) depends on configured permissions — not on their
persona. Covers:

- A developer on a team at a company
- A group of developers on a shared side project or shared OSS repository

Cares about:
- Shared, reviewable configuration
- Per-user credentials that do not leak across teammates
- Not stepping on an in-flight workflow a teammate started
- Repeatable workflows that live alongside the project

## `observer`

A user who watches Tanren activity but does not develop. Coverage and action
scope depend on configured permissions. Covers:

- A CTO or director tracking velocity across projects and teams
- An engineering manager monitoring throughput on their team
- A stakeholder (technical program manager, product owner) watching a project's
  health

Cares about:
- Read-only visibility at the scope they are granted
- Velocity, throughput, and health signals
- Not being asked to configure anything to get value

## `any`

Special value, not a persona. Use in behavior frontmatter only when the
capability is genuinely identical for every persona above.
