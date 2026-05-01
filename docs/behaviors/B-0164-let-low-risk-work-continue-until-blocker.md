---
schema: tanren.behavior.v0
id: B-0164
title: Let low-risk work continue until a blocker
area: autonomy-controls
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can let low-risk work continue until a blocker so Tanren can make progress without constant supervision.

## Preconditions

- The work is within an autonomy level that permits continued execution.
- Required readiness, dependency, and policy checks have passed.

## Observable outcomes

- Tanren continues eligible work without asking at every intermediate step.
- Progress remains visible while the work runs.
- Tanren pauses and asks for input when a blocker, approval gate, or boundary is reached.

## Out of scope

- Continuing work after a configured stop boundary.
- Suppressing source signals needed for later review.

## Related

- B-0002
- B-0005
- B-0162
