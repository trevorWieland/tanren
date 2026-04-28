---
id: B-0223
title: Connect an organization-owned provider integration
area: integration-management
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can connect an organization-owned provider integration so Tanren can use shared provider access under organization policy.

## Preconditions

- The user has permission to manage organization provider integrations.
- The external provider supports an organization-owned connection.

## Observable outcomes

- The connection records provider, owner scope, reachable resources, permissions, and health.
- Shared credentials or app installations are stored without revealing secret values.
- Projects can use the integration only within configured policy.

## Out of scope

- Replacing user-owned provider connections.
- Granting access to every project automatically.

## Related

- B-0126
- B-0225
- B-0227
