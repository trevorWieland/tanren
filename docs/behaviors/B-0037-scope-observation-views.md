---
id: B-0037
title: Scope observation views by project, grouping, or team
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can choose the slice of work an observation view
reflects — a single project, a milestone, an initiative, a team, or the
entire account — so that the view answers the question they are actually
asking.

## Preconditions

- Has visibility scope over at least one project, milestone, initiative, or
  team.

## Observable outcomes

- The user can select a project, a milestone, an initiative, a team, or the
  whole account as the scope of any observation view.
- Scope selections can be combined where meaningful (for example, a specific
  milestone within a specific project).
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
