---
schema: tanren.behavior.v0
id: B-0172
title: Reuse prior user preferences when shaping work
area: decision-memory
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can have Tanren reuse prior preferences when shaping work so repeated guidance does not have to be restated every time.

## Preconditions

- The user or project has recorded preferences relevant to the work.
- The user has visibility into the preference being applied.

## Observable outcomes

- Tanren can apply relevant preferences to planning, shaping, or review suggestions.
- The user can see when a preference influenced a suggestion.
- Preferences can be changed or ignored for a specific scope when policy allows.

## Out of scope

- Treating preferences as hidden instructions.
- Letting preferences override explicit project policy.

## Related

- B-0048
- B-0098
- B-0151
