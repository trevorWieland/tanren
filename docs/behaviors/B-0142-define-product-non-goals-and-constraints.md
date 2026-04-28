---
id: B-0142
title: Define product non-goals and constraints
area: product-discovery
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can define product non-goals and constraints so Tanren avoids planning work that conflicts with deliberate boundaries.

## Preconditions

- An active project is selected.
- The user has permission to edit product planning context.

## Observable outcomes

- Non-goals and constraints are recorded as product planning context.
- Candidate work that conflicts with a boundary can be flagged for review.
- Changed or removed boundaries remain traceable.

## Out of scope

- Enforcing legal or compliance policy by itself.
- Blocking work without showing the relevant boundary.

## Related

- B-0098
- B-0158
- B-0165
