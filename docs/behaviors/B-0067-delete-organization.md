---
id: B-0067
title: Delete an organization
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` who holds the permission to delete an organization can
disband it, so that an organization that is no longer needed ceases to
exist along with its membership, configuration, and policy.

## Preconditions

- The user holds the permission to delete the organization.
- Before deletion completes, the user has made a decision for every
  project owned by the organization (see Observable outcomes below).

## Observable outcomes

- Before the deletion takes effect the user is shown every project owned
  by the organization and, for each, chooses one of: detach the project
  to an account they designate, or delete the project along with the
  organization.
- Every member is notified that the organization is being deleted.
- Pending invitations to the organization are cancelled.
- After deletion, the organization no longer exists; its accounts lose
  organization membership but the accounts themselves remain.
- Projects that were detached remain accessible under their new owning
  accounts; projects that were deleted are removed (not recoverable from
  within Tanren — prior export per B-0063 is the only recovery path).

## Out of scope

- Undeleting or recovering a deleted organization.
- Transferring ownership of the organization itself to another
  organization.

## Related

- B-0030
- B-0059
- B-0060
- B-0063
- B-0066
