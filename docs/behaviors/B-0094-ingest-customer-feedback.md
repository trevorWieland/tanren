---
schema: tanren.behavior.v0
id: B-0094
title: Ingest customer feedback into candidate work
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can capture customer feedback as candidate work so product input is not lost before prioritization.

## Preconditions

- An active project is selected.
- The user has permission to add intake items.

## Observable outcomes

- Feedback is recorded with its source and context.
- The feedback can be linked to existing specs or turned into new candidate specs.
- The user can defer feedback without deleting it.

## Out of scope

- Customer relationship management.
- Automatically promising delivery to the customer.

## Related

- B-0018
- B-0093
- B-0098
