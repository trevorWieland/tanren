---
id: B-0065
title: See and configure who has access to an organization
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see who has access to an organization and
what organization-level permissions each member holds, so that the
access model is transparent to everyone in the organization. A `team-dev`
with the permission to manage organization access can grant, revoke, or
adjust member access.

## Preconditions

- For viewing: the user is a member of the organization.
- For changing: the user additionally holds the permission to manage
  organization access.

## Observable outcomes

- The user can see a list of every member of the organization and, for
  each member, the organization-level permissions they hold.
- The member list is visible to every member of the organization — no
  one's membership is hidden from other members.
- A permitted user can add a member with chosen organization-level
  permissions, modify an existing member's organization permissions, or
  revoke organization access entirely.
- When granting organization access, the user may select individual
  permissions or apply a role template (B-0038) that grants its bundled
  permissions at that moment.
- Project access within the organization is managed separately per
  project via B-0031; organization-level membership does not itself grant
  access to any project.
- Every change is attributed to the user who made it and visible in the
  organization's change history (B-0042).

## Out of scope

- Project-level access (covered by B-0031).
- Cross-organization access templates.
- Role- or group-based rules that span multiple organizations.

## Related

- B-0031
- B-0038
- B-0042
- B-0044
- B-0060
