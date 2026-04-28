---
id: B-0190
title: Review a proposed planning change
area: product-planning
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can review a proposed planning change so shared product direction changes deliberately.

## Preconditions

- A planning proposal exists.
- The user has visibility into the proposal and affected planning context.

## Observable outcomes

- The user can see the proposed change, rationale, affected work, and supporting evidence.
- The proposal can be accepted, rejected, revised, or left open according to configured permissions.
- The review outcome is attributed and remains linked to the proposal.

## Out of scope

- Requiring every reviewer to hold the same permissions.
- Turning review comments into accepted planning changes without approval.

## Related

- B-0115
- B-0189
- B-0191
