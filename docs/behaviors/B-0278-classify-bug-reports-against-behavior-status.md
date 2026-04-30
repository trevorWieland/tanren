---
schema: tanren.behavior.v0
id: B-0278
title: Classify bug reports against behavior status
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can classify bug reports against behavior status so defects route through product intent instead of becoming unstructured interruptions.

## Preconditions

- A bug report or defect signal exists.
- A behavior catalog exists for the affected product or project.

## Observable outcomes

- The report can be classified as false alarm, out of scope, missing accepted behavior, misaligned accepted behavior, implemented but unasserted behavior, or asserted behavior regression.
- The classification records rationale and links the report to affected behavior or product planning context.
- Follow-up work can be routed to behavior planning, roadmap revision, assertion work, or implementation repair.

## Out of scope

- Treating every bug report as accepted product scope.
- Proving regression solely from the report text.
- Automatically changing accepted behavior without product review.

## Related

- B-0094
- B-0097
- B-0161
- B-0181
- B-0189
- B-0276
- B-0277
