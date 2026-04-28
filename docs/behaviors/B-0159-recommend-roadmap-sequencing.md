---
id: B-0159
title: Recommend roadmap sequencing with tradeoffs
area: prioritization
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can review recommended roadmap sequencing with tradeoffs so planned work has an explainable order.

## Preconditions

- Roadmap or candidate work exists.
- The user has permission to edit or propose roadmap ordering.

## Observable outcomes

- Tanren proposes an order with rationale tied to visible product context.
- Tradeoffs and risks of the recommended order are visible.
- The user can accept, revise, or reject the recommendation.

## Out of scope

- Scheduling work without regard for declared dependencies.
- Treating the recommendation as approval to execute gated work.

## Related

- B-0092
- B-0112
- B-0158
