---
id: B-0274
title: Rotate or revoke API keys
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with API key management permission can rotate or revoke API keys so
automation access can be changed or stopped without exposing secret values.

## Preconditions

- An API key exists in the selected scope.
- The user has permission to rotate or revoke API keys for that scope.

## Observable outcomes

- The user can rotate an API key and receive the replacement secret only at
  creation time.
- The user can revoke an API key so future requests using it are denied.
- Existing usage metadata remains available after rotation or revocation.
- Rotation and revocation are attributed and visible in audit history.

## Out of scope

- Recovering an existing API key secret value.
- Revoking unrelated provider credentials unless separately integrated.
- Treating key rotation as proof that all external clients updated safely.

## Related

- B-0128
- B-0129
- B-0221
- B-0222
