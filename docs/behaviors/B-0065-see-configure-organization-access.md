---
id: B-0065
title: See and manage existing members' access to an organization
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see who has access to an organization and
what organization-level permissions each member holds, so that the access
model is transparent. A `team-dev` with the permission to manage
organization access can adjust an existing member's permissions.

## Preconditions

- For viewing: the user is a member of the organization.
- For changing: the user additionally holds the permission to manage
  organization access.

## Observable outcomes

- The user can see a list of every member of the organization and, for
  each member, the organization-level permissions they hold.
- The member list is visible to every member — no one's membership or
  organization-level permissions are hidden from other members.
- A permitted user can modify an existing member's organization-level
  permissions, including by applying a role template (B-0038), or revoke
  organization access entirely.
- Project access within the organization is managed separately per
  project via B-0031; organization-level membership does not itself grant
  access to any project.
- Every change is attributed to the user who made it and visible in the
  organization's change history (B-0042).

## Out of scope

- Adding new members — new membership is granted only by invitation (see
  B-0044). This behavior covers existing members.
- Removing members entirely from the organization (see B-0060 for
  involuntary removal and B-0059 for voluntary leave).
- Project-level access (covered by B-0031).
- Role definitions that span multiple organizations.

## Related

- B-0031
- B-0038
- B-0042
- B-0044
- B-0059
- B-0060
- B-0066
- B-0067
