---
schema: tanren.behavior.v0
id: B-0058
title: Cancel a loop
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can cancel a running or paused implementation
loop so that Tanren stops working on the spec and the spec becomes
available for a fresh start.

## Preconditions

- A loop exists in a running or paused state.
- The user has permission to act on the loop.

## Observable outcomes

- The user can cancel the loop; Tanren stops working on the spec.
- A cancelled loop cannot be resumed — unlike B-0007 (pause), cancellation
  is final for that loop.
- The spec returns to a state where a new loop can be started on it (per
  B-0001 or B-0002), subject to other preconditions such as dependencies
  (B-0017).
- The cancellation is attributed to the user who performed it and appears
  in the loop's action history (B-0014).

## Out of scope

- Preserving partial progress to resume later — pause (B-0007) is the
  mechanism for that.
- Automatically cancelling loops based on timeouts or policy — this
  behavior is user-initiated.

## Related

- B-0001
- B-0007
- B-0014
- B-0017
