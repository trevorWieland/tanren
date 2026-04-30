---
schema: tanren.behavior.v0
id: B-0288
title: Review behavior catalog coherence
area: product-planning
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can review behavior catalog coherence so the
behavior library stays complete, non-overlapping, and product-shaped.

## Preconditions

- A behavior catalog exists.
- Product planning context exists for comparison.
- The user has permission to edit or review behavior planning context.

## Observable outcomes

- Missing, duplicate, overlapping, oversized, stale, or implementation-shaped
  behavior entries are surfaced with rationale.
- Proposed catalog changes preserve accepted behavior identity and history.
- The index remains coherent with behavior files and product areas.

## Out of scope

- Changing verification status without assessment or executable behavior proof.
- Rewriting product vision or architecture as part of coherence review.
- Silently repurposing accepted behavior IDs.

## Related

- B-0276
- B-0277
- B-0283
