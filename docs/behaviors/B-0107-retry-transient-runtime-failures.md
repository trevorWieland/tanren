---
id: B-0107
title: Retry transient runtime failures without duplicating visible work
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

A user or Tanren worker can retry transient runtime failures so temporary infrastructure or provider issues do not duplicate user-visible work.

## Preconditions

- A runtime failure is classified as transient and retryable.
- Retry policy permits another attempt.

## Observable outcomes

- Tanren records each retry attempt.
- The user can see that retrying is happening or happened.
- Duplicate specs, tasks, or visible loop ownership are not created by retry.

## Out of scope

- Retrying non-transient policy denials.
- Infinite retry loops.

## Related

- B-0105
- B-0106
