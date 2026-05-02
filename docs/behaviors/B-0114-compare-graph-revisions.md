---
schema: tanren.behavior.v0
id: B-0114
title: Compare graph revisions
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can compare revisions of a work graph so plan evolution remains reviewable.

## Preconditions

- At least two graph revisions exist.
- The user has visibility of the graph.

## Observable outcomes

- The user can compare node and dependency changes between revisions.
- Each revision identifies why it was created.
- The comparison preserves historical context even after newer revisions exist.

## Out of scope

- Editing historical revisions.
- Showing unauthorized graph nodes.

## Related

- B-0110
- B-0113
