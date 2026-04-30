---
schema: tanren.behavior.v0
id: B-0095
title: Ingest meeting notes into candidate work
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can turn meeting notes into candidate work so decisions and requests become traceable product inputs.

## Preconditions

- An active project is selected.
- The user has permission to add intake items.

## Observable outcomes

- Meeting notes are captured with source context.
- Candidate work items can be extracted for user review.
- Accepted candidates can become specs or roadmap items.

## Out of scope

- Recording meetings.
- Accepting generated work without user review.

## Related

- B-0018
- B-0092
- B-0094
