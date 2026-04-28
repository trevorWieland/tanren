---
id: B-0216
title: Subscribe to an observer digest
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can subscribe to an observer digest so meaningful project changes reach them without constant manual checking.

## Preconditions

- The user has visibility into the chosen scope.
- Notification or digest delivery is available for at least one configured channel.

## Observable outcomes

- The user can choose scope, cadence, and categories for digest updates.
- Each digest summarizes changes, risks, blockers, shipped outcomes, and attention-worthy items within visible scope.
- The user can pause, revise, or unsubscribe from the digest.

## Out of scope

- Sending hidden details through a digest.
- Replacing urgent notifications for active blockers.

## Related

- B-0004
- B-0062
- B-0219
