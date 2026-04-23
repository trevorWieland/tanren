# Domain Concepts

Canonical terminology for behavior files. Use these exact terms consistently.
Behaviors may freely reference these concepts without re-defining them.

## Hierarchy

- **Project** — a code repository. All work is scoped to a project.
- **Spec** — one unit of work, analogous to a ticket. Belongs to exactly one
  project. The Tanren core implementation loop runs per spec and stays within
  that spec's scope.
- **Milestone** — a named grouping of specs.
- **Initiative** — a named grouping of milestones.

**Containment**: Initiative → Milestone → Spec, all within a Project.

**Orphans allowed**: a spec may exist without a milestone, and a milestone may
exist without an initiative. A spec always belongs to a project.

## External issue trackers

Tanren is the system of record for specs. External trackers (Linear, Jira,
GitHub Issues) are integrated one-way:

- Tanren may *push* issues outbound to an external tracker (for audit findings,
  PR feedback, etc.).
- An external issue can later be *shaped* into a new Tanren spec, but the spec
  is authoritative from that point on — Tanren does not mirror external state.
- A spec may *reference* external issue URLs as dependencies (read-only links).

Behaviors that involve external trackers should state this directionality
explicitly.

## Organizations

An **Organization** is a governance entity that owns a set of projects and
can enforce policy (access restrictions, mandatory rules, shared roles)
across them. Organizations set the `organizational` context for behaviors.
A work account belongs to one organization; a personal account does not
belong to any organization. Detail on organization setup and membership
lives in the configuration and credentials area.

## Roles and permissions

Tanren authorizes every action by checking **permissions**, not roles.
A **role** is a grant-time convenience — a named template that bundles a
set of permissions so they can be granted in one action. Applying a role
to a person grants the bundled permissions as individual grants at that
moment. Subsequent access checks resolve on permissions only.

Editing or deleting a role does not retroactively alter the permissions
of people who already hold grants derived from it.

## Accounts

A user signs into Tanren with an **Account** that scopes which projects and
organizations they can access. One person typically holds multiple accounts —
for example, a personal account for side projects and a work account for
company projects. A single account may belong to multiple organizations; a
personal account belongs to none. Behaviors that reference projects assume a
currently active account, and when operating within an organization, a
currently selected organization. Detail on how accounts are created and
managed lives in the configuration and credentials area.

## Configuration tiers

Tanren's configuration is split into three tiers, each with different
ownership and visibility:

- **User-tier** — configuration and credentials tied to a specific developer,
  such as personal authentication tokens for agent providers. Never shared
  across users.
- **Project-tier** — configuration specific to a single project, such as
  gate commands, standard folder conventions, and project-scoped secrets.
  Shared with everyone who has access to the project.
- **Organization-tier** — configuration that applies across every project in
  an organization, typically deployment-related — for example, shared
  infrastructure secrets or organization-wide defaults. Shared with everyone
  in the organization; usually set by organization admins.

## Scopes

Scope is a permission dimension that modifies what a persona can see or act on.
Behaviors state scope as a precondition when relevant.

- **Own** — the user's own work.
- **Team** — work of other developers on the same team.
- **Cross-team** — work across teams within the same organization or
  collaboration boundary.

## Contexts

Every behavior declares one or both contexts in frontmatter:

- **`personal`** — no external governance. Side projects, OSS, individual work.
- **`organizational`** — subject to organizational policy, budgets, audit, or
  access control.
