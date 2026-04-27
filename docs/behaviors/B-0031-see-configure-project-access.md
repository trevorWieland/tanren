---
id: B-0031
title: See and configure who has access to a project
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see who has access to a project and what
visibility and permissions each person holds, so that the access model is
transparent to everyone involved. A `team-dev` with the required permission
can grant, revoke, or adjust another person's access.

## Preconditions

- For viewing: the user has visibility of the project.
- For changing: the user additionally has permission to manage the project's
  access list. In organizational contexts this permission may be restricted.

## Observable outcomes

- The user can see a list of every person with access to the project and,
  for each person, their visibility scope (concepts.md defines the scope
  vocabulary) and the specific permissions they hold — for example,
  whether they may assist or take over teammates' loops per B-0012, or
  manage project access.
- The access list is visible to every person who has visibility of the
  project — no one's access is hidden from other members.
- A permitted user can add a person with a chosen visibility and permission
  set, modify an existing person's access, or revoke access entirely.
- When granting, the user may select individual permissions directly or
  apply a role template (B-0038) that grants its bundled permissions at
  that moment.
- Every change is attributed to the user who made it and visible in the
  project's change history (B-0042).

## Out of scope

- Organization-wide access policies that constrain what a project can grant
  (covered in a later governance area).
- Group- or role-based access (e.g. "everyone in the platform team") — this
  behavior covers individual access.
- Cross-project access templates.

## Related

- B-0010
- B-0011
- B-0012
- B-0014
