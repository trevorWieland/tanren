---
schema: tanren.behavior.v0
id: B-0222
title: See API key and service account usage
area: configuration
personas: [operator, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see API key and service account usage so automation access can be audited without revealing secrets.

## Preconditions

- The user has visibility into API key or service account metadata for the selected scope.

## Observable outcomes

- Usage views show last use, affected scope, integration, and action category.
- Secret values are never displayed in usage views.
- Unused, stale, or suspicious usage can be routed to rotation, revocation, or investigation.

## Out of scope

- Exposing request payloads that contain hidden data.
- Treating usage visibility as permission to change access.

## Related

- B-0127
- B-0220
- B-0221
