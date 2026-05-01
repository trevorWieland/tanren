---
schema: tanren.behavior.v0
id: B-0036
title: See a per-builder breakdown of contribution
area: observation
personas: [team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` with the required permission can see observation
metrics broken down by individual builder, so that coaching, load
balancing, and recognition have concrete signal behind them.

## Preconditions

- Has visibility scope over the builders being viewed.
- Has the specific permission to see per-builder breakdowns, which is
  granted separately from project-level visibility because it exposes
  individual-level data.

## Observable outcomes

- Velocity, throughput, quality, and health signals (B-0032, B-0033, B-0034)
  are available broken down per builder in addition to the team and
  project-level views.
- The breakdown respects the same time windows (B-0035) and scope selections
  (B-0037) as other observation behaviors.
- A user without the per-builder permission sees team-level and
  project-level aggregates only; individual breakdowns are not shown even
  incidentally.
- Every builder can see their own metrics without requiring the per-builder
  permission.

## Out of scope

- Ranking or scoring builders automatically.
- Hiding the existence of the per-builder view — its existence is visible
  to all, only the data is permission-gated.

## Related

- B-0031
- B-0032
- B-0033
- B-0034
