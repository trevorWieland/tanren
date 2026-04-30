---
schema: tanren.behavior.v0
id: B-0201
title: See team attention load without performance surveillance
area: team-coordination
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see team attention load without performance surveillance so overloaded work queues and review bottlenecks are visible without turning Tanren into individual monitoring.

## Preconditions

- The user has visibility into a shared project or grouping.
- Attention-worthy work exists across multiple builders or review queues.

## Observable outcomes

- Tanren summarizes blocked, waiting, review-needed, and approval-needed work by queue or scope.
- The view helps redistribute or prioritize attention without ranking people by productivity.
- Per-person details appear only when relevant to visible work and configured permissions.

## Out of scope

- Measuring individual productivity as a primary behavior.
- Exposing private workload or activity outside policy.

## Related

- B-0036
- B-0037
- B-0187
