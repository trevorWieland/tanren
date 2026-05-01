---
schema: tanren.behavior.v0
id: B-0088
title: Manage project-scoped secrets
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can manage secrets scoped to a project so work can access required credentials without exposing secret values.

## Preconditions

- An active project is selected.
- The user has permission to manage project secrets.

## Observable outcomes

- The user can add, update, and remove project-scoped secrets.
- Secret values are not shown after storage.
- Use of a secret is visible without revealing its value.

## Out of scope

- User-owned credentials.
- Organization-scoped secrets.

## Related

- B-0048
- B-0126
- B-0127
