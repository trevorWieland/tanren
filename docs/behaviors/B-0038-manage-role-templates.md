---
id: B-0038
title: Manage roles as permission templates
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` with permission to manage roles can create, edit, and delete
named roles that bundle a set of permissions, so that grantors can apply a
coherent bundle in one action instead of selecting individual permissions
each time.

## Preconditions

- The user has permission to manage roles at the scope they are editing —
  either a specific project or the organization.

## Observable outcomes

- The user can define a role with a name and a set of permissions it bundles.
- Roles can exist at the project level (scoped to one project) or at the
  organization level (available across all projects in the organization).
- Applying a role to a person grants the role's permissions as individual
  grants at that moment; access checks always resolve on permissions, never
  on role membership.
- Editing a role's contents does not retroactively change the permissions of
  people who were previously granted the role.
- Deleting a role does not revoke permissions that were previously granted
  through it.

## Out of scope

- Nested roles (roles that include other roles).
- Dynamic or attribute-based role computation.
- Automatic re-application of role updates to existing grantees.

## Related

- B-0031
