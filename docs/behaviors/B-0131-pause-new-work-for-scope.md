---
schema: tanren.behavior.v0
id: B-0131
title: Pause new work for a project or organization
area: operations
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with permission can pause new Tanren work for a project or organization so operators can stop additional execution during incidents, maintenance, or policy review.

## Preconditions

- The user has permission to control work intake for the scope.

## Observable outcomes

- New work does not start while the pause is active.
- Users can see that the scope is paused and why.
- Existing work is handled according to the selected pause policy.

## Out of scope

- Cancelling every in-flight loop automatically.
- Pausing unrelated scopes.

## Related

- B-0058
- B-0130
- B-0132
