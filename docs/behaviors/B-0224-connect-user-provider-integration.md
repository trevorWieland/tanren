---
id: B-0224
title: Connect a user-owned provider integration
area: integration-management
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can connect a user-owned provider integration so Tanren can use personal provider access without sharing the underlying credential.

## Preconditions

- The user has permission to connect provider access for their account or project use.
- The provider supports user-owned authorization.

## Observable outcomes

- The connection is visibly user-owned and not shared as project or organization access.
- Tanren records provider, scope, capability, health, and expiration metadata without exposing secret values.
- Work that needs the connection can explain when the user's personal access is missing or expired.

## Out of scope

- Turning personal access into a shared secret.
- Requiring every provider to support personal access.

## Related

- B-0125
- B-0199
- B-0225
