---
id: B-0256
title: Return machine-readable validation errors
area: integration-contract
personas: [integration-client]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can receive machine-readable validation errors so external automation can respond without parsing human prose.

## Preconditions

- A client request fails validation through a public Tanren contract.

## Observable outcomes

- Errors include a stable category, affected field or scope where safe, and remediation hint.
- Human-readable text may accompany the machine-readable error but is not the only contract.
- Sensitive details remain redacted even in structured error data.

## Out of scope

- Guaranteeing identical wording across interfaces.
- Revealing hidden resources through validation details.

## Related

- B-0185
- B-0239
- B-0261
