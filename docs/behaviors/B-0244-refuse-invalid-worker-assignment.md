---
schema: tanren.behavior.v0
id: B-0244
title: Refuse invalid or unauthorized worker assignments
area: runtime-actor-contract
personas: [operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can refuse invalid or unauthorized assignments so Tanren does not execute work without a valid scope.

## Preconditions

- A worker receives or attempts to resume an assignment.

## Observable outcomes

- Missing, expired, malformed, revoked, or policy-denied assignments are refused.
- The refusal records a Tanren-level reason visible to users with runtime visibility.
- No work artifacts, external side effects, or credential use occur after refusal.

## Out of scope

- Letting a worker infer missing scope from local state.
- Hiding assignment refusal from operational views.

## Related

- B-0185
- B-0239
- B-0243
