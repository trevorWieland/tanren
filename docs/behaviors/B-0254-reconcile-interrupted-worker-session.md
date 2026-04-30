---
schema: tanren.behavior.v0
id: B-0254
title: Resume or reconcile interrupted worker sessions
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can resume or reconcile an interrupted session so Tanren understands whether work continued, stopped, or needs recovery.

## Preconditions

- A worker session was interrupted by crash, disconnect, lease loss, provider failure, or host failure.

## Observable outcomes

- Tanren records whether the session resumed, reconciled to terminal state, or requires recovery.
- Work state, source signals, access grants, and retries remain linked to the original assignment.
- Users can see what was recovered and what remains uncertain.

## Out of scope

- Pretending interrupted work completed without source signals.
- Creating a fresh visible work item when reconciliation can link to the original assignment.

## Related

- B-0105
- B-0107
- B-0250
