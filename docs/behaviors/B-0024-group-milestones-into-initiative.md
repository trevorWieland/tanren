---
id: B-0024
title: Group milestones into an initiative
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can create an initiative and assign milestones to
it, so that a broader program of work spanning multiple milestones can be
tracked and viewed together.

## Preconditions

- An active project is selected.
- The user has permission to create initiatives and assign milestones. In
  organizational contexts this permission may be restricted.

## Observable outcomes

- The user can create an initiative with a name within a project.
- The user can assign milestones to and unassign milestones from an
  initiative.
- A milestone may belong to at most one initiative at a time; a milestone
  may also exist without any initiative.
- The initiative is visible to anyone with visibility of the project and is
  usable as a grouping in B-0016.
- The user can rename or delete an initiative. Deleting an initiative does
  not delete its milestones; they simply become unassigned.

## Out of scope

- Timelines, due dates, or program roadmaps — an initiative in Tanren is a
  named grouping, not a dated program.
- Cross-project initiatives.

## Related

- B-0016
- B-0023
