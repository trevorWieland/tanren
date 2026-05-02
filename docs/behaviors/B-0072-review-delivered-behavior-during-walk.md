---
schema: tanren.behavior.v0
id: B-0072
title: Review delivered behavior during a walk
area: review-merge
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can review the delivered behavior for a spec during a walk so that acceptance is based on observed outcomes rather than implementation claims.

## Preconditions

- A walk has been started for the spec.
- The user has permission to participate in the walk.

## Observable outcomes

- The user can see the behaviors and acceptance criteria being walked.
- The user can review source signals, notes, and demonstrations associated with the walk.
- The user can record walk observations without accepting or rejecting the work yet.

## Out of scope

- Code review of source diffs.
- Merging the work.

## Related

- B-0006
- B-0073
- B-0074
