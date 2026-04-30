---
schema: tanren.behavior.v0
id: B-0129
title: Revoke a credential or secret
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with permission can revoke a credential or secret so Tanren stops using access that should no longer be available.

## Preconditions

- The credential or secret exists.
- The user has permission to revoke it.

## Observable outcomes

- Future work cannot use the revoked credential or secret.
- Affected work reports missing access rather than silently using stale access.
- The revocation is attributed and visible in relevant history.

## Out of scope

- Revoking credentials at the external provider unless explicitly integrated.
- Deleting historical usage records.

## Related

- B-0125
- B-0126
- B-0128
