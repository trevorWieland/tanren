---
id: B-0059
title: Leave an organization
personas: [any]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
status: draft
supersedes: []
---

## Intent

A user can voluntarily leave an organization their account belongs to, so
that they stop appearing in the organization's member list and lose access
to its projects without having to delete the account itself.

## Preconditions

- The user's active account belongs to the organization being left.
- The user is not the last remaining admin of the organization. If so, an
  admin transfer or deletion of the organization must happen first.

## Observable outcomes

- After leaving, the account is no longer a member of the organization and
  can no longer access the organization's projects.
- The user's account, any personal data, and memberships in other
  organizations are unaffected.
- Any ownership, assignments, or in-flight work the user held in the
  organization is surfaced before the action completes so nothing is
  orphaned silently.
- The action is attributed and appears in the organization's permission
  change history (B-0042).

## Out of scope

- Deleting the account itself.
- Rejoining automatically after leaving — rejoining requires a new
  invitation per B-0044.

## Related

- B-0042
- B-0044
- B-0045
- B-0060
