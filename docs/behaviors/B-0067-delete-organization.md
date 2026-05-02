---
schema: tanren.behavior.v0
id: B-0067
title: Delete an organization
area: governance
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` who holds the permission to delete an organization can
disband it, so that an organization that is no longer needed ceases to
exist along with its membership, configuration, and policy.

## Preconditions

- The user holds the permission to delete the organization.
- Before deletion completes, the user has made a decision for every
  project owned by the organization (see Observable outcomes below).

## Observable outcomes

- Before the deletion takes effect the user is shown every project owned
  by the organization and must resolve the project's disposition through
  B-0272.
- Every member is notified that the organization is being deleted.
- Pending invitations to the organization are cancelled.
- After deletion, the organization no longer exists; its accounts lose
  organization membership but the accounts themselves remain.
- Projects resolved through B-0272 follow their chosen disposition.

## Out of scope

- Undeleting or recovering a deleted organization.
- Transferring ownership of the organization itself to another
  organization.
- Choosing disposition for each organization-owned project.

## Related

- B-0030
- B-0059
- B-0060
- B-0063
- B-0066
- B-0272
