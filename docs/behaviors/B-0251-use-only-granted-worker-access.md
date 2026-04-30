---
schema: tanren.behavior.v0
id: B-0251
title: Use only granted credentials and environment access
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

An `agent-worker` can use only granted credentials and environment access so execution remains inside its approved boundary.

## Preconditions

- The worker has an active assignment.
- The assignment identifies allowed credentials, resources, tools, and environment access.

## Observable outcomes

- Access attempts outside the grant are denied and recorded.
- Users can see the access boundary and denied access category without exposing secret values.
- Work that needs additional access pauses or fails with an explainable reason.

## Out of scope

- Letting workers read ambient credentials not granted by Tanren.
- Revealing hidden resources through access-denied details.

## Related

- B-0104
- B-0234
- B-0239
