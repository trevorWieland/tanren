---
id: B-0117
title: Create a pull request from walked work
area: review-merge
personas: [solo-builder, team-builder, integration-client]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can create a pull request from accepted walked work so delivered changes enter source-control review.

## Preconditions

- Walked work has been accepted.
- The project is connected to a supported source-control destination.
- The user or Tanren has permission to create the pull request.

## Observable outcomes

- A pull request is created with links back to the spec and evidence.
- The spec shows the pull request link and state.
- Failure to create the pull request leaves the accepted walk state intact with an actionable error.

## Out of scope

- Manually editing the pull request content outside Tanren.
- Merging the pull request.

## Related

- B-0073
- B-0057
- B-0118
