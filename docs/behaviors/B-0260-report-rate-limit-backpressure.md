---
id: B-0260
title: Report rate limit and backpressure state
area: integration-contract
personas: [integration-client, observer, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can receive rate limit and backpressure state so external automation can slow down or retry safely.

## Preconditions

- A client request reaches a quota, rate, queue, or backpressure boundary.

## Observable outcomes

- The response identifies whether the boundary is rate limit, quota, queue pressure, maintenance, or policy.
- Retry guidance is machine-readable when retry is allowed.
- Operators can see aggregate pressure without exposing unrelated client details.

## Out of scope

- Encouraging clients to bypass limits.
- Exposing other clients' secrets or private workloads.

## Related

- B-0130
- B-0230
- B-0256
