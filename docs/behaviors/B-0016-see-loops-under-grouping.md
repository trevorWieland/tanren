---
schema: tanren.behavior.v0
id: B-0016
title: See all active loops under a milestone or initiative
area: team-coordination
personas: [team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` can view every active loop under a chosen milestone
or initiative, so that they can see adjacent in-flight work and coordinate
without being surprised by overlapping effort.

## Preconditions

- A milestone or initiative exists the user has visibility of.
- Has visibility scope over the loops to be listed. Loops the user cannot see
  are excluded from the view but their existence is not implied.

## Observable outcomes

- The user can select a milestone or initiative and see a list of the active
  loops grouped under it.
- Each listed loop shows its spec, its owner, and its current state — the
  same summary available from B-0003.
- The user can drill from the list into any individual loop they have
  visibility of.
- This view is informational — it does not block or prevent any action on its
  own.

## Out of scope

- Blocking a new loop start based on what is running under the same grouping
  (Tanren does not prevent parallel loops across different specs in the same
  milestone or initiative).
- Historical views of finished loops under a grouping.

## Related

- B-0003
- B-0009
