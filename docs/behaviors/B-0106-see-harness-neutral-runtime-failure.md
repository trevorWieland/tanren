---
id: B-0106
title: See runtime failure evidence in a harness-neutral form
area: runtime-substrate
personas: [solo-builder, team-builder, observer, operator]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see runtime failure evidence in a consistent Tanren vocabulary so failures are understandable across different harnesses and environments.

## Preconditions

- Tanren work has failed during runtime execution.
- The user has visibility of the work.

## Observable outcomes

- The failure view uses Tanren-level categories such as policy, credentials, environment, timeout, or provider failure.
- Harness-specific details are available only as supporting evidence.
- The user can tell whether the failure is retryable or needs human action.

## Out of scope

- Hiding all provider detail.
- Debugging provider source code.

## Related

- B-0101
- B-0105
- B-0107
