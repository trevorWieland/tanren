---
schema: tanren.behavior.v0
id: B-0119
title: Route review feedback into follow-up work
area: review-merge
personas: [solo-builder, team-builder, integration-client]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or Tanren worker can route actionable review feedback into follow-up work so review comments are resolved through the same controlled workflow as other changes.

## Preconditions

- A pull request has review feedback.
- The feedback is visible to Tanren and the acting user or worker.

## Observable outcomes

- Actionable feedback can become follow-up tasks or specs.
- Follow-up work links back to the source feedback.
- Non-actionable feedback can be classified without creating work.

## Out of scope

- Arguing with reviewers automatically.
- Changing external comments without permission.

## Related

- B-0118
- B-0120
