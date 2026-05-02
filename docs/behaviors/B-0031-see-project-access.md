---
schema: tanren.behavior.v0
id: B-0031
title: See who has access to a project
area: governance
personas: [team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` can see who has access to a project and what
visibility and permissions each person holds, so that the access model is
transparent to everyone involved.

## Preconditions

- The user has visibility of the project.

## Observable outcomes

- The user can see a list of every person with access to the project and,
  for each person, their visibility scope (concepts.md defines the scope
  vocabulary) and the specific permissions they hold — for example,
  whether they may assist or take over teammates' loops per B-0012, or
  manage project access.
- The access list is visible to every person who has visibility of the
  project — no one's access is hidden from other members.
- The user can see whether access was granted directly or through a role
  template, without treating the role as the thing authorization checks.
- Access changes are attributable through the project's change history
  (B-0042).

## Out of scope

- Granting, revoking, or adjusting project access.
- Organization-wide access policies that constrain what a project can grant.
- Cross-project access templates.

## Related

- B-0010
- B-0011
- B-0012
- B-0014
- B-0202
