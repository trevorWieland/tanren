---
id: B-0060
title: Remove a member from an organization
area: governance
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with the required permission can remove another member from
an organization so that the departing or unwanted member loses access to
the organization's projects without needing their cooperation.

## Preconditions

- The user has permission to remove members from the organization.
- Removal would not leave the organization without any member holding a
  given administrative permission. If the member being removed is the
  last holder of any administrative permission, another holder must be
  appointed first.

## Observable outcomes

- After removal, the removed account is no longer a member of the
  organization and can no longer access the organization's projects.
- Any ownership or in-flight work the removed member held in the
  organization is surfaced before the action completes so the remover can
  reassign or cancel it.
- The removed member is notified that they have been removed.
- The removal is attributed to the user who performed it and appears in
  the organization's permission change history (B-0042).

## Out of scope

- Deleting the removed member's account.
- Removing someone from a project without removing them from the
  organization — that is handled by B-0031 (project-level access).

## Related

- B-0031
- B-0042
- B-0059
