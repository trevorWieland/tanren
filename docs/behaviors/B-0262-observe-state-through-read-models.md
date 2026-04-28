---
id: B-0262
title: Observe state through read models or subscriptions
area: integration-contract
personas: [integration-client, observer, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can observe Tanren state through read models or subscriptions so external systems can stay synchronized without scraping interfaces.

## Preconditions

- The client has permission to read the selected scope.
- The requested state is available through a public machine contract.

## Observable outcomes

- The client can request current state, changes after a cursor, or subscribed event streams where supported.
- Read results include stable resource identities and freshness or cursor metadata.
- Hidden state is omitted or redacted according to the client's permission boundary.

## Out of scope

- Exposing internal database shapes.
- Requiring every UI view to have an identical machine read model.

## Related

- B-0183
- B-0257
- B-0258
