---
schema: tanren.behavior.v0
id: B-0192
title: Claim ownership of work or review
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can claim ownership of work or review so shared responsibilities have an accountable current owner.

## Preconditions

- The work or review is visible to the user.
- The user has permission to claim ownership for the item.

## Observable outcomes

- The item shows the user as the current owner or reviewer.
- Prior ownership and claim history remain visible where policy allows.
- Work that cannot be claimed explains why.

## Out of scope

- Claiming work outside the user's scope.
- Treating ownership as exclusive permission to make every decision.

## Related

- B-0011
- B-0015
- B-0193
