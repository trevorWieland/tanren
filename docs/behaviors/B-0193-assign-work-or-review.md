---
schema: tanren.behavior.v0
id: B-0193
title: Assign work or review to another builder
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can assign work or review to another builder so responsibility can be routed explicitly.

## Preconditions

- The target work and assignee are visible within the project.
- The user has permission to assign the item.

## Observable outcomes

- The assignment identifies the assignee, assigner, reason, and time.
- The assignee can see the assignment as attention-worthy work.
- Reassignment preserves prior assignment history.

## Out of scope

- Granting the assignee permissions they do not already hold.
- Treating assignment as proof that the assignee accepted the work.

## Related

- B-0009
- B-0187
- B-0192
