---
id: B-0249
title: Honor cancellation and access revocation
area: runtime-actor-contract
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can honor cancellation and access revocation so Tanren-controlled execution stops when users or policy require it.

## Preconditions

- A worker has active work, access, or an execution lease.
- Cancellation or access revocation has been issued for the assignment or scope.

## Observable outcomes

- The worker stops using revoked access and stops or pauses affected work according to policy.
- The affected work records whether cancellation completed, failed, or needs recovery.
- Evidence needed to understand what happened remains available.

## Out of scope

- Continuing work after cancellation by switching credentials.
- Deleting evidence to make cancellation look clean.

## Related

- B-0058
- B-0235
- B-0254
