---
schema: tanren.behavior.v0
id: B-0205
title: See roadmap progress against product goals
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see roadmap progress against product goals so delivery status remains tied to product intent.

## Preconditions

- The project has roadmap items or product goals.
- The user has visibility into the relevant planning context.

## Observable outcomes

- Tanren shows roadmap items grouped or filtered by product goal.
- Progress distinguishes planned, in progress, blocked, shipped, deferred, and superseded work.
- Gaps between roadmap progress and product goals are visible for review.

## Out of scope

- Treating a roadmap item as successful only because code merged.
- Exposing hidden roadmap details.

## Related

- B-0092
- B-0098
- B-0180
