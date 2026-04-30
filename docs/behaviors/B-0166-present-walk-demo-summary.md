---
schema: tanren.behavior.v0
id: B-0166
title: Present a walk or demo summary before acceptance
area: walk-acceptance
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see a walk or demo summary before acceptance so review focuses on delivered behavior rather than implementation claims.

## Preconditions

- Work has reached walk or acceptance review.
- The user has visibility into the spec and walk acceptance record.

## Observable outcomes

- The summary names the intended behavior, demonstrated outcome, and supporting source references.
- Missing source signals or skipped demonstrations are visible.
- The user can continue to accept, reject, or request follow-up work.

## Out of scope

- Replacing user acceptance where acceptance is required.
- Hiding failed or incomplete source signals from the summary.

## Related

- B-0072
- B-0073
- B-0167
