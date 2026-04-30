---
schema: tanren.behavior.v0
id: B-0110
title: See the planned graph of work
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see the planned graph of work for a roadmap item or spec so dependencies and execution order are understandable before or during delivery.

## Preconditions

- A plan exists for the work.
- The user has visibility of the planned work.

## Observable outcomes

- The user can see graph nodes and dependency relationships.
- The user can distinguish ready, blocked, running, and completed graph nodes.
- The graph links back to the product or spec context it came from.

## Out of scope

- Exposing internal scheduler data structures.
- Editing the graph without permission.

## Related

- B-0077
- B-0093
- B-0111
