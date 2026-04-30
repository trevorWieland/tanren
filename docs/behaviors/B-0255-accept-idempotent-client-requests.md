---
schema: tanren.behavior.v0
id: B-0255
title: Accept idempotent client create and update requests
area: integration-contract
personas: [integration-client, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can send idempotent create and update requests so retries from external automation do not duplicate Tanren state.

## Preconditions

- The client is authenticated and has permission for the requested action.
- The request includes a stable idempotency key or equivalent external identity.

## Observable outcomes

- Repeated equivalent requests return the same created or updated Tanren resource.
- Conflicting reuse of an idempotency key is rejected with a machine-readable error.
- The idempotency result is attributable to the client and visible in audit where allowed.

## Out of scope

- Making non-idempotent provider side effects safe without explicit recovery behavior.
- Accepting unauthenticated client writes.

## Related

- B-0185
- B-0221
- B-0264
