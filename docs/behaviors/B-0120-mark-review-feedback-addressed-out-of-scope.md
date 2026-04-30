---
schema: tanren.behavior.v0
id: B-0120
title: Mark review feedback as addressed or out of scope
area: review-merge
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can mark review feedback as addressed or out of scope so Tanren knows whether remaining comments block merge readiness.

## Preconditions

- Review feedback exists for a pull request.
- The user has permission to triage feedback.

## Observable outcomes

- Each feedback item has a visible disposition.
- Addressed feedback links to the work or source signals that resolved it.
- Out-of-scope feedback can be routed to backlog or ignored with a reason.

## Out of scope

- Deleting reviewer comments.
- Bypassing required reviewer approval.

## Related

- B-0119
- B-0121
