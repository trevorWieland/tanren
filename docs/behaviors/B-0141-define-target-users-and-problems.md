---
schema: tanren.behavior.v0
id: B-0141
title: Define target users and problems
area: product-discovery
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can define target users and their problems so roadmap and spec work stays anchored to real needs.

## Preconditions

- An active project is selected.
- The user has permission to edit product planning context.

## Observable outcomes

- Target user groups and problem statements are recorded separately.
- Each problem can be linked to roadmap items, specs, or candidate work.
- Open uncertainty about users or problems remains visible for later discovery.

## Out of scope

- Replacing customer research or user interviews.
- Assigning implementation priority by itself.

## Related

- B-0079
- B-0098
- B-0143
