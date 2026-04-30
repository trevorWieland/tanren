---
schema: tanren.behavior.v0
id: B-0250
title: Avoid duplicate visible work on retry
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

An `agent-worker` can retry assigned work without creating duplicate visible work so transient failures do not confuse users or corrupt planning state.

## Preconditions

- A retryable worker assignment failed, timed out, or was interrupted.
- Retry policy permits another attempt.

## Observable outcomes

- Retries reuse or link to the original assignment, work item, and idempotency context.
- Duplicate specs, tasks, reviews, pull requests, or visible ownership records are not created by retry.
- Users can see each attempt and why retry occurred.

## Out of scope

- Hiding repeated failed attempts.
- Retrying non-idempotent external side effects without configured recovery behavior.

## Related

- B-0107
- B-0254
- B-0264
