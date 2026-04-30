---
schema: tanren.behavior.v0
id: B-0220
title: Create and manage service accounts
area: governance
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can create and manage service accounts so non-human automation can act through accountable, scoped identities.

## Preconditions

- The user has permission to manage service accounts for the selected scope.

## Observable outcomes

- Service accounts have names, scopes, permissions, owners, and purpose metadata.
- Service account creation, change, suspension, and deletion are attributed.
- Service accounts are distinguishable from human accounts in access and audit views.

## Out of scope

- Treating service accounts as personas.
- Granting service accounts unrestricted access by default.

## Related

- B-0202
- B-0203
- B-0221
