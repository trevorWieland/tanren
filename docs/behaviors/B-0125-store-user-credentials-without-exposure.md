---
schema: tanren.behavior.v0
id: B-0125
title: Store user credentials without exposing secret values
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can store credentials needed by Tanren without later exposing secret values to other users, logs, or routine views.

## Preconditions

- The user is signed into an account.
- The user has a credential for a supported integration or harness.

## Observable outcomes

- The credential can be added or updated.
- The stored secret value is not displayed after submission.
- Tanren can show credential presence, scope, and last-updated metadata without showing the secret.

## Out of scope

- Sharing user credentials with teammates.
- Displaying raw credential values.

## Related

- B-0048
- B-0099
- B-0127
