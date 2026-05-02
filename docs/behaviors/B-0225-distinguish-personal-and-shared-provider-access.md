---
schema: tanren.behavior.v0
id: B-0225
title: Distinguish personal provider access from shared provider access
area: integration-management
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can distinguish personal provider access from shared provider access so integration behavior is understandable and safe.

## Preconditions

- The selected scope has at least one external provider connection.
- The user has visibility into provider connection metadata.

## Observable outcomes

- Provider connections identify whether access is user-owned, project-owned, organization-owned, service-account, or worker-scoped.
- Views explain which work can use each connection without showing secret values.
- Missing provider access explains whether a personal or shared connection is needed.

## Out of scope

- Revealing secret values or provider tokens.
- Assuming shared access can replace personal access in every provider.

## Related

- B-0199
- B-0223
- B-0224
