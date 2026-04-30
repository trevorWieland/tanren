---
schema: tanren.behavior.v0
id: B-0112
title: See why Tanren chose the next available work
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see why Tanren chose the next available work so scheduling decisions are explainable.

## Preconditions

- Tanren has selected work to run from a graph or queue.
- The user has visibility of the selected work.

## Observable outcomes

- The user can see the selection rationale at a product level.
- The rationale references readiness, dependencies, policy, lanes, or priority as applicable.
- The explanation does not expose unauthorized work details.

## Out of scope

- Tuning every scheduler heuristic from the view.
- Guaranteeing deterministic ordering for equally eligible work.

## Related

- B-0002
- B-0110
- B-0111
