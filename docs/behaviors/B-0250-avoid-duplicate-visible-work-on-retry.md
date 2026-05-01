---
schema: tanren.behavior.v0
id: B-0250
title: Avoid duplicate visible work on retry
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can retry assigned work without creating duplicate visible work so transient failures do not confuse users or corrupt planning state.

## Preconditions

- A retryable worker assignment failed, timed out, or was interrupted.
- Retry policy permits another attempt.

## Observable outcomes

- Retries link to the original assignment and work item with stable identity so the work being retried is unambiguous.
- Duplicate specs, tasks, reviews, pull requests, or visible ownership records are not created by retry.
- Users can see each attempt and why retry occurred.

## Out of scope

- Hiding repeated failed attempts.
- Repeating external side effects that cannot be safely repeated without explicit recovery behavior.

## Related

- B-0107
- B-0254
- B-0264
