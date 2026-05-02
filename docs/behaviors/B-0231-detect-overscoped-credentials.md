---
schema: tanren.behavior.v0
id: B-0231
title: Detect over-scoped credentials or integrations
area: configuration
personas: [observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can detect over-scoped credentials or integrations so external access can be reduced before it becomes an operational risk.

## Preconditions

- Credential, secret, API key, or provider connection metadata is visible to the user.

## Observable outcomes

- Tanren identifies access that appears broader than configured need or policy.
- Findings explain affected scope, capability, and source signals without revealing secret values.
- The user can route the concern to rotation, revocation, policy change, or accepted exception.

## Out of scope

- Automatically reducing access without approval.
- Claiming provider permissions are safe when Tanren cannot inspect them.

## Related

- B-0127
- B-0227
- B-0233
