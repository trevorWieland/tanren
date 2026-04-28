---
id: B-0258
title: Deliver webhooks with retry, ordering, and dedupe
area: integration-contract
personas: [integration-client, operator]
interfaces: [api]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can receive webhooks with retry, ordering, and dedupe metadata so event consumers can process Tanren events reliably.

## Preconditions

- A webhook endpoint is configured for the client's scope.
- Events matching the endpoint subscription occur.

## Observable outcomes

- Webhook deliveries include stable event identity, timestamp, resource identity, and ordering or cursor metadata where available.
- Failed deliveries retry according to visible policy and expose delivery state to permitted users.
- Duplicate deliveries are identifiable by the receiver.

## Out of scope

- Guaranteeing exactly-once delivery to external systems.
- Delivering hidden events outside the endpoint's configured scope.

## Related

- B-0240
- B-0255
- B-0262
