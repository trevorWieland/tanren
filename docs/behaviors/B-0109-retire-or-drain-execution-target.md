---
id: B-0109
title: Retire or drain an execution target
area: runtime-substrate
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can retire or drain an execution target so new work stops landing there without losing track of in-flight work.

## Preconditions

- An execution target exists.
- The user has permission to manage the target.

## Observable outcomes

- Draining prevents new work from starting on the target while visible in-flight work completes or moves.
- Retiring removes the target from future placement.
- Affected users can see why the target is unavailable.

## Out of scope

- Killing in-flight work without explicit cancellation.
- Deleting historical execution evidence.

## Related

- B-0105
- B-0108
- B-0130
