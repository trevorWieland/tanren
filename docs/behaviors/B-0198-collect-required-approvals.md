---
schema: tanren.behavior.v0
id: B-0198
title: Collect required approvals for a gated action
area: governance
personas: [team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can collect required approvals for a gated action so policy-bound work can proceed once the configured conditions are met.

## Preconditions

- A gated action is waiting for approval.
- The user has permission to request, provide, or manage approvals for the action.

## Observable outcomes

- Tanren records each approval, rejection, expiration, or withdrawal with attribution.
- The action proceeds only after configured approval conditions are satisfied.
- Denied or incomplete approvals leave actionable state on the affected work.

## Out of scope

- Defining approval policy in this behavior.
- Treating a persona as an approval authority.

## Related

- B-0040
- B-0163
- B-0197
