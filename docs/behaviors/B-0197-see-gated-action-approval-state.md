---
schema: tanren.behavior.v0
id: B-0197
title: See approval state for a gated action
area: governance
personas: [team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see approval state for a gated action so they know why work is waiting and what approval remains.

## Preconditions

- A visible action is gated by project or organization policy.
- The user has visibility into the action or its containing work.

## Observable outcomes

- Tanren shows whether the action is pending, approved, rejected, expired, or blocked.
- The approval state explains required approval conditions without exposing hidden approver details.
- The state links back to the work, policy, and source signals visible to the user.

## Out of scope

- Granting approval authority.
- Hard-coding approver identity by persona.

## Related

- B-0115
- B-0163
- B-0198
