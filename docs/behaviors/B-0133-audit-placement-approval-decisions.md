---
schema: tanren.behavior.v0
id: B-0133
title: Audit placement and approval decisions
area: operations
personas: [team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with visibility can audit placement and approval decisions so governed execution remains explainable after work runs.

## Preconditions

- The user has audit visibility for the organization or project.
- Placement or approval decisions have occurred.

## Observable outcomes

- The user can see who or what made each decision.
- The decision records cite relevant policy or approval state.
- Secret values and unauthorized work details remain hidden.

## Out of scope

- Changing past decisions.
- Showing data outside the user's visibility scope.

## Related

- B-0040
- B-0081
- B-0104
