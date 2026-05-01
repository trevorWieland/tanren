---
schema: tanren.behavior.v0
id: B-0255
title: Accept retry-safe client create and update requests
area: integration-contract
personas: [integration-client, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can send create and update requests that are safe to retry so retries from external automation do not duplicate Tanren state.

## Preconditions

- The client is authenticated and has permission for the requested action.
- The request carries a stable retry-safety identity (such as a client-supplied unique request identifier or equivalent external resource identity) that lets Tanren recognize repeated equivalent requests.

## Observable outcomes

- Repeated equivalent requests return the same created or updated Tanren resource.
- Reusing the same retry-safety identity for an inconsistent request is rejected with a machine-readable error.
- The retry-safety result is attributable to the client and visible in audit where allowed.

## Out of scope

- Making external provider side effects safe to repeat without explicit recovery behavior.
- Accepting unauthenticated client writes.

## Related

- B-0185
- B-0221
- B-0264
