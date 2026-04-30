---
schema: tanren.behavior.v0
id: B-0287
title: Review proactive analysis recommendations
area: proactive-analysis
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can review proactive analysis recommendations so automated discovery is
interpreted before it changes product planning or execution.

## Preconditions

- A proactive analysis run has produced findings, risks, or recommendations.
- The user has visibility into the affected scope.

## Observable outcomes

- The user can see each recommendation with source, source references, provenance,
  affected behavior or planning context, and suggested next action.
- The user can accept, reject, defer, or request more investigation with
  rationale.
- Accepted recommendations route through planning rather than directly mutating
  active specs.

## Out of scope

- Replacing human product judgment for ambiguous recommendations.
- Treating insufficiently supported findings as accepted work without review.
- Running the analysis itself.

## Related

- B-0189
- B-0190
- B-0279
- B-0286
