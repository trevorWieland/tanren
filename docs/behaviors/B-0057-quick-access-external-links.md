---
schema: tanren.behavior.v0
id: B-0057
title: Quick-access external links from a spec to its ticket and pull request
area: external-tracker
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can open a spec and jump directly
from that view to the originating ticket in the connected external tracker
and to the pull request in the connected git service, so that moving
between Tanren and the external systems takes one action rather than a
hunt for the right link.

## Preconditions

- The user is viewing a spec they have visibility of.
- For ticket access: the spec has an originating ticket (B-0018) and the
  project has a connected external tracker (B-0052).
- For pull request access: the spec has one or more associated pull
  requests produced by implementation loops on the connected repository
  (B-0025).

## Observable outcomes

- From a spec view, the user can open the spec's originating ticket in the
  external tracker in a single action.
- From a spec view, the user can open any associated pull request in the
  connected git service in a single action.
- When a spec has multiple pull requests (for example, from successive
  loops), the user can see them listed and pick one.
- Links are visible to every user with visibility of the spec, not only to
  the spec's owner.
- The same quick access is available on every supported interface,
  including on a phone.

## Out of scope

- Showing the content of the ticket or pull request inline within Tanren —
  this behavior navigates out, it does not mirror.
- Editing a ticket or pull request from within Tanren.

## Related

- B-0018
- B-0021
- B-0025
- B-0052
- B-0056
