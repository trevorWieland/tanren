---
schema: tanren.behavior.v0
id: B-0089
title: See project configuration change history
area: configuration
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with project visibility can see project configuration changes so they can understand when working rules changed and who changed them.

## Preconditions

- The user has visibility of the project.

## Observable outcomes

- The user can see configuration changes in chronological order.
- Each entry identifies the actor, time, and affected setting without leaking secret values.
- The history remains available after later configuration changes.

## Out of scope

- Editing history entries.
- Showing secret values.

## Related

- B-0042
- B-0049
- B-0088
