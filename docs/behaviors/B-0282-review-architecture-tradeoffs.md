---
schema: tanren.behavior.v0
id: B-0282
title: Review architecture tradeoffs
area: architecture-planning
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can review architecture tradeoffs so design choices that affect product,
delivery, security, or operations are understood before they guide work.

## Preconditions

- An architecture proposal or architecture change exists.
- The user has visibility into the affected planning scope.

## Observable outcomes

- The user can see the alternatives considered, selected direction, rationale,
  risks, and affected behaviors or roadmap work.
- Decisions that require human judgment remain pending until reviewed.
- Rejected alternatives remain visible enough to avoid repeated rediscovery.

## Out of scope

- Automatically accepting product-impacting architecture changes.
- Requiring every small implementation choice to become an architecture
  decision.
- Replacing the behavior catalog as the source of product capability.

## Related

- B-0170
- B-0171
- B-0190
- B-0281
