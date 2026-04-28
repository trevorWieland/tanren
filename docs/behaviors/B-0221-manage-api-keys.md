---
id: B-0221
title: Create and scope API keys
area: governance
personas: [operator, integration-client]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can create and scope API keys so public automation access starts with
explicit ownership, permissions, and boundaries.

## Preconditions

- The user has permission to create API keys for the selected account, project,
  organization, or service account.

## Observable outcomes

- API keys are created with explicit scope, permissions, expiration, and owner.
- Key secret values are shown only at creation or rotation time.
- Rotation and revocation affect future use and remain visible in audit history.

## Out of scope

- Rotating or revoking existing API keys.
- Displaying existing API key secret values.
- Using personas as API key permission bundles.

## Related

- B-0128
- B-0129
- B-0222
- B-0274
