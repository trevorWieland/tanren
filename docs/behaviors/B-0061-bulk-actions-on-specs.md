---
schema: tanren.behavior.v0
id: B-0061
title: Perform bulk actions on multiple specs
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can select multiple specs at once and apply a
supported action to all of them — archive, move to or from the backlog,
assign to a milestone, and similar lifecycle actions — so that managing a
set of related specs does not require repeating the same action
spec-by-spec.

## Preconditions

- The user has visibility of the specs they are selecting.
- The user has the permission required for the chosen action on every spec
  in the selection.

## Observable outcomes

- The user can select multiple specs in a list or grouping view and choose
  a bulk action from the same view.
- Supported bulk actions include at least: archive (B-0022), move to
  backlog (B-0020), mark ready (B-0019), and assign to a milestone
  (B-0023).
- The user sees a preview of what will change before confirming.
- Specs in the selection that do not meet the preconditions for the chosen
  action are skipped, and the user is told which ones and why.
- Each successful change is attributed individually in spec history the
  same way it would be if performed one at a time.

## Out of scope

- Bulk starting of implementation loops — each loop requires a deliberate
  start (B-0001) and honors B-0013.
- Cross-project bulk actions.

## Related

- B-0019
- B-0020
- B-0022
- B-0023
