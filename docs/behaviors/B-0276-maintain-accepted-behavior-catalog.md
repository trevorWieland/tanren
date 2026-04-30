---
schema: tanren.behavior.v0
id: B-0276
title: Maintain the accepted behavior catalog
area: product-planning
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can maintain the accepted behavior catalog so product intent remains a durable, portable contract for future work.

## Preconditions

- A product brief or planning context exists.
- The user has permission to edit behavior planning context.

## Observable outcomes

- The user can propose, accept, revise, deprecate, supersede, or remove behavior entries with rationale.
- Behavior changes preserve stable IDs and history for accepted or asserted behavior.
- Behavior entries describe actor-visible capability and observable outcome without depending on a specific implementation.

## Out of scope

- Creating roadmap DAG nodes.
- Writing implementation source signals into behavior files.
- Silently repurposing accepted behavior IDs.

## Related

- B-0079
- B-0092
- B-0170
- B-0189
- B-0190
