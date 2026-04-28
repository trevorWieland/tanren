---
id: B-0074
title: Reject walked work and route follow-up work
area: review-merge
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can reject walked work with specific follow-up needs so that Tanren routes the spec back into work instead of marking it done.

## Preconditions

- The spec is in a walk state.
- The user has permission to reject the walk.

## Observable outcomes

- The rejection records the behavior gap or concern.
- Tanren creates or routes follow-up work for the spec.
- The spec does not become done until follow-up work is completed and walked again.

## Out of scope

- Discarding the spec entirely.
- Rejecting work without recording why.

## Related

- B-0006
- B-0072
- B-0018
