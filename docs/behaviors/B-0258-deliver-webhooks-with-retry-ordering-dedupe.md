---
schema: tanren.behavior.v0
id: B-0258
title: Deliver webhooks reliably so event consumers can process them without ambiguity
area: integration-contract
personas: [integration-client, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can receive webhook event notifications reliably so event consumers can process Tanren events without ambiguity, loss, or undetected duplication.

## Preconditions

- A webhook endpoint is configured for the client's scope.
- Events matching the endpoint subscription occur.

## Observable outcomes

- Webhook deliveries include stable event identity, timestamp, resource identity, and ordering information where available.
- Failed deliveries are retried according to a visible policy, and delivery state is visible to permitted users.
- Duplicate deliveries can be recognized as duplicates by the receiver.

## Out of scope

- Guaranteeing exactly-once delivery to external systems.
- Delivering hidden events outside the endpoint's configured scope.

## Related

- B-0240
- B-0255
- B-0262
