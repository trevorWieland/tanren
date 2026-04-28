---
id: B-0027
title: See all projects in an account with attention indicators
area: project-setup
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can see every project in their
currently active account at a glance, together with clear indicators of which
projects and which specs need their attention, so that nothing slips while
they are juggling several projects.

## Preconditions

- The user is signed into an account with at least one project.

## Observable outcomes

- The user can see a list of every project in the active account.
- Each project shows a summary of its current state, including whether any
  spec in the project needs attention (e.g. a blocker, an error, a walk
  pending, a notification they have not acted on).
- Project-level attention indicators aggregate spec-level attention — if no
  spec in the project needs attention, neither does the project.
- The view is usable on a phone: attention indicators are legible on a small
  screen and the user can quickly drill from a project into the spec that
  needs attention.

## Out of scope

- Cross-account aggregation — the view is per active account.
- Custom filters, groupings, or saved dashboards beyond the default layout.

## Related

- B-0003
- B-0021
- B-0028
