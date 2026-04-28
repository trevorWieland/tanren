---
id: B-0257
title: Negotiate API and schema versions
area: integration-contract
personas: [integration-client, operator]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can negotiate API and schema versions so external automation can remain compatible across Tanren upgrades.

## Preconditions

- The client uses a public Tanren machine contract.

## Observable outcomes

- The client can discover supported versions for the relevant contract.
- Unsupported versions fail with a machine-readable compatibility error.
- Deprecation or migration guidance is visible before removal where possible.

## Out of scope

- Supporting every historical contract forever.
- Requiring internal storage schemas to match public schemas.

## Related

- B-0134
- B-0183
- B-0256
