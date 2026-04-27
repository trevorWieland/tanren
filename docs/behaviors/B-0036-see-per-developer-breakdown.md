---
id: B-0036
title: See a per-developer breakdown of contribution
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` with the required permission can see observation
metrics broken down by individual developer, so that coaching, load
balancing, and recognition have concrete signal behind them.

## Preconditions

- Has visibility scope over the developers being viewed.
- Has the specific permission to see per-developer breakdowns, which is
  granted separately from project-level visibility because it exposes
  individual-level data.

## Observable outcomes

- Velocity, throughput, quality, and health signals (B-0032, B-0033, B-0034)
  are available broken down per developer in addition to the team and
  project-level views.
- The breakdown respects the same time windows (B-0035) and scope selections
  (B-0037) as other observation behaviors.
- A user without the per-developer permission sees team-level and
  project-level aggregates only; individual breakdowns are not shown even
  incidentally.
- Every developer can see their own metrics without requiring the
  per-developer permission.

## Out of scope

- Ranking or scoring developers automatically.
- Hiding the existence of the per-developer view — its existence is visible
  to all, only the data is permission-gated.

## Related

- B-0031
- B-0032
- B-0033
- B-0034
