---
schema: tanren.behavior.v0
id: B-0105
title: Stop or recover interrupted execution
area: runtime-substrate
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can stop or recover interrupted execution so work state remains
understandable after cancellation, crash, or lease loss.

## Preconditions

- Active or interrupted work exists.
- The user has permission to stop or recover it.

## Observable outcomes

- Tanren records whether execution stopped, failed, or recovered.
- Recovered work continues from a known state or asks for human decision.
- Recovery does not create duplicate visible work.
- When the interruption involved a worker, the recovered state remains linked
  to that worker's prior assignment so accountability is preserved.

## Out of scope

- Guaranteeing recovery from every infrastructure failure.
- Silently discarding source signals.

## Related

- B-0058
- B-0107
- B-0130
- B-0254
