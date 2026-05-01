---
schema: tanren.behavior.v0
id: B-0011
title: Transfer ownership of a loop to another team-builder
area: team-coordination
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can transfer ownership of an in-flight loop to another `team-builder`,
so that the new owner becomes responsible for the work going forward.

## Preconditions

- The loop is not already closed.
- Either the current owner is initiating a handoff, or another `team-builder` is
  taking over under permission to do so (see B-0012).

## Observable outcomes

- After transfer, the new owner is the primary recipient of the loop's
  notifications.
- The previous owner retains visibility of the loop but is no longer the
  primary owner.
- The transfer is attributed and visible to anyone with visibility of the
  loop, so the change of ownership is traceable.
- A loop can only have one owner at a time.

## Out of scope

- Assisting without taking ownership (see B-0010).
- Governance of when takeovers are allowed (see B-0012).

## Related

- B-0010
- B-0012
