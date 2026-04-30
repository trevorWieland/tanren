---
id: B-0116
title: Link evidence artifacts to graph nodes
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see which evidence artifacts prove each graph node so planning, execution, checks, and outcomes remain connected.

## Preconditions

- A graph node has produced or consumed evidence.
- The user has visibility of the graph node.

## Observable outcomes

- Plans, patches, tests, checks, findings, demos, and outcomes link to the graph node they support.
- The user can navigate from a node to its visible evidence.
- Evidence remains linked after replanning.

## Out of scope

- Storing large binary artifacts in behavior docs.
- Showing evidence the user cannot access.

## Related

- B-0080
- B-0110
- B-0113
