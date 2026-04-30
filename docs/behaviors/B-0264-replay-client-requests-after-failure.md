---
schema: tanren.behavior.v0
id: B-0264
title: Safely replay client requests after failure
area: integration-contract
personas: [integration-client, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can safely replay requests after client, network, or Tanren failure so automation can recover without corrupting state.

## Preconditions

- A prior client request may have partially completed, timed out, or returned an uncertain result.
- The replay includes the original idempotency or resource identity context.

## Observable outcomes

- Replay returns the existing result, completes the pending request, or reports a conflict in machine-readable form.
- Tanren does not create duplicate visible work, duplicate external side effects, or conflicting ownership records.
- Operators can audit replay attempts and outcomes.

## Out of scope

- Making unsafe non-idempotent requests replayable by default.
- Hiding partial failure source signals.

## Related

- B-0250
- B-0255
- B-0256
