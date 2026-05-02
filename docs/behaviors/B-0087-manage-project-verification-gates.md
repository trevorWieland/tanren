---
schema: tanren.behavior.v0
id: B-0087
title: Manage project verification gates
area: configuration
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can configure project verification gates so work is checked consistently before readiness or completion.

## Preconditions

- An active project is selected.
- The user has permission to manage verification gates.

## Observable outcomes

- The project records required gates for relevant work phases.
- Users can see which gates apply to a spec or task.
- Tanren blocks readiness when required gates have not passed.

## Out of scope

- Writing the gate implementation.
- Ignoring mandatory organization policy.

## Related

- B-0049
- B-0080
