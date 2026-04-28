---
id: B-0265
title: Coordinate candidate implementations for one spec
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can coordinate multiple candidate
implementations for one spec so competing solutions can be compared before one
is selected as the accepted path.

## Preconditions

- The spec is shaped and eligible for implementation.
- The user has permission to run candidate implementation work.
- Project policy allows candidate comparison for the spec.

## Observable outcomes

- Each candidate implementation is visibly tied to the same spec and has its
  own owner, evidence, and terminal outcome.
- Users can compare candidate evidence, changed surfaces, risks, and acceptance
  results before choosing a candidate.
- The selected candidate becomes the continuation path for review and merge.
- Rejected candidates remain traceable without becoming active product work.

## Out of scope

- Starting accidental parallel loops outside a coordinated comparison.
- Automatically choosing a winner without evidence or required approval.
- Merging multiple candidate branches into one result without explicit follow-up.

## Related

- B-0013
- B-0158
- B-0169
- B-0171
