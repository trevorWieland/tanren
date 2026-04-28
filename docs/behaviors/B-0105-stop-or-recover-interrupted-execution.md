---
id: B-0105
title: Stop or recover interrupted execution
area: runtime-substrate
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or Tanren worker can stop or recover interrupted execution so work state remains understandable after cancellation, crash, or lease loss.

## Preconditions

- Active or interrupted work exists.
- The actor has permission or worker authority to stop or recover it.

## Observable outcomes

- Tanren records whether execution stopped, failed, or recovered.
- Recovered work continues from a known state or asks for human decision.
- No duplicate visible work is created by recovery.

## Out of scope

- Guaranteeing recovery from every infrastructure failure.
- Silently discarding evidence.

## Related

- B-0058
- B-0107
- B-0130
