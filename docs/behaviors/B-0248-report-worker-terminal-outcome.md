---
id: B-0248
title: Report terminal worker outcomes
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator, integration-client]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can report terminal outcomes so assigned work finishes with a durable success, failure, cancellation, timeout, or blocked state.

## Preconditions

- The worker has an active assignment.
- The assignment has reached a terminal result or cannot continue.

## Observable outcomes

- The terminal outcome is linked to the assignment, attempt, evidence, and affected work.
- Users can distinguish success, failure, cancellation, timeout, and blocked outcomes.
- A terminal outcome triggers the next configured planning, review, retry, or recovery behavior.

## Out of scope

- Marking product work accepted solely because a worker reports success.
- Hiding partial evidence after terminal failure.

## Related

- B-0106
- B-0107
- B-0253
