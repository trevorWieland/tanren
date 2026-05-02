---
schema: tanren.behavior.v0
id: B-0047
title: Switch the active organization within an account
area: governance
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user whose active account belongs to more than one organization can
switch which organization is currently in focus, so that views and actions
scoped to an organization apply to the one they intend.

## Preconditions

- The active account belongs to at least two organizations.

## Observable outcomes

- The user can list every organization their active account belongs to and
  select any of them as the active organization.
- Switching organizations changes which projects are listed and which
  organization-level policies and configuration apply.
- Switching is available on every supported interface, including on a
  phone.
- If the active account belongs to no organizations (a personal account),
  this behavior has no effect and no organization-scoped actions are
  available.

## Out of scope

- Viewing projects from multiple organizations simultaneously.
- Pinning or reordering organizations within an account.

## Related

- B-0045
- B-0046
