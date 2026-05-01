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
- The replay carries the same retry-safety identity (such as the original client-supplied unique request identifier or the resource identity it acted on) so Tanren can recognize it as a replay of the same intent.

## Observable outcomes

- A replay returns the existing result, completes the pending request, or reports a conflict in machine-readable form.
- Tanren does not create duplicate visible work, repeat external side effects that were not declared safe to repeat, or produce conflicting ownership records.
- Operators can audit replay attempts and outcomes.

## Out of scope

- Replaying requests whose external side effects are not safe to repeat, without explicit recovery behavior.
- Hiding partial failure source signals.

## Related

- B-0250
- B-0255
- B-0256
