---
id: B-0243
title: Receive a scoped worker assignment
area: runtime-actor-contract
personas: [operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can receive a scoped assignment so Tanren-controlled execution has explicit work, phase, capability, and evidence boundaries.

## Preconditions

- Work is eligible for dispatch.
- Placement, harness, credential, and policy checks allow a worker assignment.

## Observable outcomes

- The assignment identifies work scope, phase or intent, allowed tools, environment, credentials, expiration, and evidence obligations.
- Users with visibility can see that a worker assignment exists without seeing secret values.
- Missing assignment context prevents the worker from starting work.

## Out of scope

- Treating phase names as runtime actor identities.
- Granting worker access outside the assignment.

## Related

- B-0102
- B-0234
- B-0251
