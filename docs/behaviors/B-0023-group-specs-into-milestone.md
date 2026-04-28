---
id: B-0023
title: Group specs into a milestone
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can create a milestone and assign specs to it, so
that a cohesive set of work can be tracked, viewed, and reported on together.

## Preconditions

- An active project is selected.
- The user has permission to create milestones and assign specs in the
  project. In organizational contexts this permission may be restricted.

## Observable outcomes

- The user can create a milestone with a name within a project.
- The user can assign specs to and unassign specs from a milestone.
- A spec may belong to at most one milestone at a time; a spec may also exist
  without any milestone.
- The milestone is visible to anyone with visibility of the project and is
  usable as a grouping in B-0016.
- The user can rename or delete a milestone. Deleting a milestone does not
  delete its specs; they simply become unassigned.

## Out of scope

- Timelines, due dates, or roadmap views — a milestone in Tanren is a named
  grouping, not a dated commitment.
- Cross-project milestones.

## Related

- B-0016
- B-0024
