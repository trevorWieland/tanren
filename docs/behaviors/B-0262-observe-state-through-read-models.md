---
schema: tanren.behavior.v0
id: B-0262
title: Observe Tanren state from external systems
area: integration-contract
personas: [integration-client, observer, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can observe Tanren state so external systems can stay synchronized without scraping interfaces meant for human use.

## Preconditions

- The client has permission to read the selected scope.
- The requested state is available through a public machine contract.

## Observable outcomes

- The client can request current state, changes since a known point in time, or streamed updates where supported.
- Read results include stable resource identities and ordering or freshness information.
- Hidden state is omitted or redacted according to the client's permission boundary.

## Out of scope

- Exposing internal storage shapes.
- Requiring every human-facing view to be available identically as a machine contract.

## Related

- B-0183
- B-0257
- B-0258
