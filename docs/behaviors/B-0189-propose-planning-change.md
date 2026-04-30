---
schema: tanren.behavior.v0
id: B-0189
title: Propose a planning change without accepting it
area: product-planning
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can propose a change to mission, roadmap, standards, specs, or graph planning without immediately making it accepted shared context.

## Preconditions

- The user has permission to propose planning changes for the project.
- The affected planning context is visible to the user.

## Observable outcomes

- The proposal records the intended change, rationale, affected context, and author.
- Accepted shared context remains unchanged until the proposal is accepted by the configured process.
- Other users with visibility can review the proposal.

## Out of scope

- Bypassing required review for shared planning context.
- Treating proposal permission as permission to accept the proposal.

## Related

- B-0092
- B-0170
- B-0190
