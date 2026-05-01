---
schema: tanren.behavior.v0
id: B-0121
title: Merge accepted work
area: review-merge
personas: [solo-builder, team-builder, operator, integration-client]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or authorized Tanren worker can merge accepted work so completed, reviewed changes land in the project repository.

## Preconditions

- The work has an accepted walk.
- Required pull request, CI, review, and policy gates are satisfied.
- The actor has permission to merge.

## Observable outcomes

- The merge is performed or queued according to project policy.
- The spec records merge completion.
- Merge failure leaves actionable source signals and does not mark the spec done.

## Out of scope

- Bypassing source-control protections.
- Merging work that has not satisfied required gates.

## Related

- B-0073
- B-0118
- B-0122
