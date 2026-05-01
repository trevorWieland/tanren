---
schema: tanren.behavior.v0
id: B-0065
title: See existing members' access to an organization
area: governance
personas: [team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` can see who has access to an organization and
what organization-level permissions each member holds, so that the access
model is transparent.

## Preconditions

- The user is a member of the organization.

## Observable outcomes

- The user can see a list of every member of the organization and, for
  each member, the organization-level permissions they hold.
- The member list is visible to every member — no one's membership or
  organization-level permissions are hidden from other members.
- The user can see whether access was granted directly or through a role
  template, without treating the role as the thing authorization checks.
- Project access within the organization is managed separately per
  project via B-0031; organization-level membership does not itself grant
  access to any project.
- Every change is attributed to the user who made it and visible in the
  organization's change history (B-0042).

## Out of scope

- Adding new members — new membership is granted only by invitation (see
  B-0044). This behavior covers existing members.
- Modifying existing members' organization-level permissions.
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
- B-0203
