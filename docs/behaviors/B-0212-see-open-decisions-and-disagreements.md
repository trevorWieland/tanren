---
schema: tanren.behavior.v0
id: B-0212
title: See open decisions and unresolved disagreements
area: observation
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see open decisions and unresolved disagreements so shared direction does not stall invisibly.

## Preconditions

- The selected scope has visible open decisions, proposals, or conflicting direction.
- The user has visibility into the relevant decision context.

## Observable outcomes

- Tanren lists open decisions, disputed alternatives, affected work, and needed next action.
- Resolved decisions are distinguishable from still-open disagreements.
- The view links to proposals, source signals, and accepted resolutions where visible.

## Out of scope

- Forcing a specific decision-making model.
- Exposing private disagreement details outside the user's scope.

## Related

- B-0189
- B-0190
- B-0191
