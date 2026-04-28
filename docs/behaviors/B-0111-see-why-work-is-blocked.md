---
id: B-0111
title: See why work is blocked by another graph node
area: planner-orchestration
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see why planned work is blocked so they know what must finish or change before it can run.

## Preconditions

- A graph node is not ready to run.
- The user has visibility of the graph node.

## Observable outcomes

- The block reason names dependency, policy, capability, lane, or backpressure at a user level.
- The user can navigate to visible blocking work.
- Hidden work is represented without leaking unauthorized details.

## Out of scope

- Bypassing required blocks.
- Showing unauthorized blocker content.

## Related

- B-0017
- B-0110
