---
id: B-0239
title: Explain why policy denied an operation
area: governance
personas: [solo-builder, team-builder, observer, operator, integration-client]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user, integration client, or agent worker can see why policy denied an operation so blocked work can be understood or routed.

## Preconditions

- A public Tanren action is denied by policy.
- The actor is allowed to know that the attempted scope exists.

## Observable outcomes

- The denial explains the policy category, affected scope, and safe next action.
- Hidden policy details are redacted without pretending the denial is arbitrary.
- Denials are recorded for audit and trend analysis.

## Out of scope

- Revealing secrets or hidden resources through denial messages.
- Treating denial explanation as permission to override policy.

## Related

- B-0185
- B-0197
- B-0238
