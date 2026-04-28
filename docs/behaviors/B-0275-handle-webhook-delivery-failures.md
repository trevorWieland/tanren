---
id: B-0275
title: Handle webhook delivery failures
area: integration-management
personas: [team-builder, operator, integration-client]
interfaces: [api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or integration client can handle webhook delivery failures so event
delivery problems are visible, attributable, and recoverable without sending
events outside their configured scope.

## Preconditions

- A webhook endpoint is configured.
- One or more deliveries have failed, timed out, or been retried.

## Observable outcomes

- Delivery failures show endpoint, event identity, attempt count, timing, and
  non-secret failure reason.
- The user can retry, pause, resume, or disable delivery for the affected
  endpoint when permitted.
- Webhook consumers receive retry and dedupe metadata for replay safety.
- Delivery failure handling is attributed and visible in integration audit
  history.

## Out of scope

- Guaranteeing delivery to unavailable external systems.
- Revealing hidden event payloads or secret signing material.
- Sending events outside the endpoint's configured subscription.

## Related

- B-0240
- B-0258
- B-0264
