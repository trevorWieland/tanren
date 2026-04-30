---
schema: tanren.behavior.v0
id: B-0037
title: Scope observation views by project or grouping
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can choose the slice of work an observation view reflects — a single
project, a milestone, an initiative, a whole organization, or the entire
account — so that the view answers the question they are actually asking.

## Preconditions

- Has visibility scope over at least one project, milestone, initiative,
  or organization.

## Observable outcomes

- The user can select a project, a milestone, an initiative, an
  organization, or the whole account as the scope of any observation
  view.
- Scope selections can be combined where meaningful (for example, a
  specific milestone within a specific project).
- Observation behaviors (B-0032, B-0033, B-0034, B-0036) all honor the
  selected scope.
- Scopes the user does not have visibility of are not listed.

## Out of scope

- Saving named scopes for reuse.
- Cross-account aggregation — scope never crosses account boundaries.

## Related

- B-0032
- B-0033
- B-0034
- B-0036
