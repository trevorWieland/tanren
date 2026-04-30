---
schema: tanren.behavior.v0
id: B-0259
title: Attribute external automation actions
area: integration-contract
personas: [integration-client, observer, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can have its actions attributed so external automation is accountable in Tanren history.

## Preconditions

- A client performs a permitted write or external status report.

## Observable outcomes

- Tanren records the client identity, credential or service account class, request source, affected scope, and action category.
- User-initiated and automation-initiated actions are distinguishable.
- Attribution remains visible in relevant history and audit views without exposing secrets.

## Out of scope

- Treating automation as an anonymous user.
- Revealing API key secret values in attribution records.

## Related

- B-0042
- B-0222
- B-0229
