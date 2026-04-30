---
schema: tanren.behavior.v0
id: B-0054
title: See outbound issues Tanren has pushed to external trackers
area: external-tracker
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` can see the issues Tanren has pushed to the
connected external tracker — issues raised by audit findings that are out
of scope for the current spec, issues raised from PR feedback, and others —
so that the team can keep a coherent view of what Tanren has filed.

## Preconditions

- The project has a connected external tracker (B-0052).
- The user has visibility of the project.

## Observable outcomes

- The user can see a list of outbound issues the project has pushed to the
  external tracker, showing each issue's title, the spec and loop that
  originated it, and the link to the tracker.
- The list can be scoped to a time window consistent with B-0035.
- The user can drill into any outbound issue and follow the link to the
  external tracker to see its current state there.

## Out of scope

- Editing the content of an issue that has already been pushed.
- Deleting an external issue from inside Tanren.

## Related

- B-0018
- B-0035
- B-0052
- B-0055
