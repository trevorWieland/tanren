---
schema: tanren.behavior.v0
id: B-0076
title: Define acceptance criteria for a spec
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can define acceptance criteria for a spec so that future work has clear behavior-level success conditions.

## Preconditions

- The spec exists and is visible to the user.
- The user has permission to edit the spec while it is still shapeable.

## Observable outcomes

- The spec records acceptance criteria separately from implementation approach.
- The criteria are visible wherever the spec state is shown.
- The criteria can be used during implementation checks and walks.

## Out of scope

- Writing technical design.
- Changing criteria after policy freezes the spec.

## Related

- B-0018
- B-0021
- B-0072
