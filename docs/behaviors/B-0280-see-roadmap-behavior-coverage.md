---
schema: tanren.behavior.v0
id: B-0280
title: See roadmap behavior coverage
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see roadmap behavior coverage so accepted behavior has a visible path toward implementation or assertion.

## Preconditions

- A behavior catalog exists.
- A roadmap or planned graph of work exists.
- The user has visibility into the selected planning scope.

## Observable outcomes

- The user can see which accepted behaviors are covered by roadmap nodes, specs, or planned behavior-proof work.
- Accepted behaviors without a planned path are visible as planning gaps.
- Roadmap items that do not complete any accepted behavior are visible as missing product rationale.

## Out of scope

- Automatically generating roadmap nodes for every uncovered behavior.
- Treating roadmap coverage as proof that a behavior is implemented or asserted.
- Requiring every accepted behavior to be scheduled immediately.

## Related

- B-0092
- B-0110
- B-0116
- B-0205
- B-0277
