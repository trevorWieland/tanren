---
schema: tanren.behavior.v0
id: B-0126
title: Store project or organization secrets without exposing secret values
area: configuration
personas: [team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with permission can store project or organization secrets so shared work can use required credentials without revealing secret values.

## Preconditions

- The user has permission to manage secrets at the chosen scope.

## Observable outcomes

- The secret can be added, updated, and removed.
- The secret value is not displayed after storage.
- Access to use the secret follows project or organization policy.

## Out of scope

- User-owned credentials.
- Printing secret values for convenience.

## Related

- B-0088
- B-0127
- B-0128
- B-0129
