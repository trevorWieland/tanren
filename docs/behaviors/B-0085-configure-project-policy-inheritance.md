---
schema: tanren.behavior.v0
id: B-0085
title: Configure project policy inheritance and overrides
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with policy permission can see and configure how project settings inherit or override organization policy so effective rules are understandable.

## Preconditions

- The project belongs to an organization.
- The user has permission to manage the relevant policy or project setting.

## Observable outcomes

- The user can see which rules are inherited from the organization.
- Allowed project overrides are explicit.
- Disallowed overrides fail with an explanation tied to the governing policy.

## Out of scope

- Cross-organization inheritance.
- Hidden policy overrides.

## Related

- B-0040
- B-0049
- B-0050
