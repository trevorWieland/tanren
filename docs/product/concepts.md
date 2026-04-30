# Product Concepts

Canonical product terminology for behavior files, roadmap work, and specs. Use
these exact terms consistently. Behaviors may freely reference these concepts
without re-defining them.

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

### Spec lifecycle states

Every spec is in exactly one of the following states at a time. Behaviors
should refer to these names rather than inventing their own.

- **Draft** — the spec is being authored (see B-0018).
- **Backlog** — the spec is shaped but not yet prioritized (see B-0020).
- **Ready** — the spec is prioritized and eligible for an implementation
  loop (see B-0019). Only ready specs may have a loop started on them.
- **Blocked** — the spec has declared dependencies that are not yet
  finished; no loop can start until they resolve (see B-0017).
- **Running** — an implementation loop is in progress on the spec (see
  B-0001).
- **Awaiting walk** — the implementation loop has completed and the spec
  is waiting for a walk before it can be considered done (see B-0006).
- **Done** — the walk has concluded; the spec is complete.
- **Archived** — the spec will not be implemented or its work is
  preserved non-destructively (see B-0022).

## External issue trackers

Tanren is the system of record for specs. External trackers (Linear, Jira,
GitHub Issues) are integrated one-way:

- Tanren may *push* issues outbound to an external tracker (for audit findings,
  PR feedback, etc.).
- When a project has a connected tracker, the spec creation flow (B-0018) can
  pull details from an external ticket to pre-fill a new spec, but the spec
  is authoritative from that point on — Tanren does not mirror external state.
- A spec may *reference* external issue URLs as dependencies (read-only links).

Behaviors that involve external trackers should state this directionality
explicitly.

## Organizations

An **Organization** is a governance entity that owns a set of projects and
can enforce policy (access restrictions, mandatory rules, shared roles)
across them. Organizations set the `organizational` context for behaviors.
A single account may belong to zero, one, or more organizations — a
personal account belongs to none, and a work account typically belongs to
one but can belong to more. Detail on organization setup and membership
lives in the configuration and credentials area.

Certain permissions are administrative by nature — they govern who can
manage organization access, policy, or configuration. The last holder of
an administrative permission in an organization cannot remove themselves
without first appointing another holder.

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

Tanren's configuration is split into four tiers, each with different
ownership and visibility:

- **User-tier** — configuration and credentials tied to a specific user,
  such as personal authentication tokens for agent providers. Never shared
  across users.
- **Account-tier** — shared defaults tied to an account, such as default
  runtime preferences, provider mappings, and project setup choices for
  multiple projects the account owns or can administer. Useful for personal
  multi-project use as well as organization-backed accounts.
- **Project-tier** — configuration specific to a single project, such as
  gate commands, standard folder conventions, and project-scoped secrets.
  Shared with everyone who has access to the project.
- **Organization-tier** — configuration that applies across every project in
  an organization, typically deployment-related — for example, shared
  infrastructure secrets or organization-wide defaults. Shared with everyone
  in the organization; set by users who hold the permission to manage
  organization configuration.

## Credential and integration ownership

Credentials and external provider connections have ownership independent of
the persona using Tanren:

- **User-owned credential** — access material tied to one user, such as a
  personal source-control token or individual coding harness credential. It is
  not shared with other users.
- **Project-owned secret** — access material scoped to one project, such as a
  project webhook secret, deployment key, or repository-specific integration
  credential.
- **Organization-owned secret** — access material governed at organization
  scope, such as a shared cloud provider credential or organization-owned app
  installation.
- **Service account credential** — access material for a non-human actor such
  as an automation client, webhook sender, or API integration.
- **Worker-scoped temporary access** — short-lived access granted to a worker
  or task for a specific execution scope.
- **External provider connection** — a connection to a system such as source
  control, an issue tracker, CI, a cloud provider, or a VM provider. A provider
  connection may be backed by any of the credential ownership modes above, but
  routine views expose provider, scope, capability, health, and usage metadata
  rather than secret values.

## Scopes

Scope is a visibility and action boundary referenced by many behaviors
as a precondition. Behaviors state scope explicitly when relevant.

- **Own** — the user's own work.
- **Project** — work or users within a project the user has access to.
  A project's "team" is the set of users with access to that project;
  when behaviors say "team" they mean the project team.
- **Organization** — work or users across every project in an organization
  the user has access to.
- **Account** — everything the active account can reach across its
  organizations and personal projects.

## Contexts

Every behavior declares one or both contexts in frontmatter:

- **`personal`** — no external governance. Side projects, OSS, individual work.
- **`organizational`** — subject to organizational policy, budgets, audit, or
  access control.
