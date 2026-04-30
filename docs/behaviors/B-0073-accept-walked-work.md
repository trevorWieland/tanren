---
schema: tanren.behavior.v0
id: B-0073
title: Accept walked work
area: review-merge
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can accept walked work so that Tanren knows the delivered behavior satisfies the spec.

## Preconditions

- The spec is in a walk state.
- The user has permission to accept the walk.
- Organization approval policy, if any, is satisfied.

## Observable outcomes

- The spec records that the walked behavior was accepted.
- Accepted work becomes eligible for pull request, merge, or completion workflows.
- The acceptance is attributed to the user or approval set that performed it.

## Out of scope

- Reviewing source diffs.
- Bypassing required approvals.

## Related

- B-0006
- B-0072
- B-0117
- B-0121
