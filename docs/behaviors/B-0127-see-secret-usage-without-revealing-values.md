---
schema: tanren.behavior.v0
id: B-0127
title: See where credentials or secrets are used without revealing them
area: configuration
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with visibility can see where credentials or secrets are used so access is auditable without exposing secret values.

## Preconditions

- A credential or secret exists.
- The user has visibility of usage metadata.

## Observable outcomes

- The user can see which work or configuration uses the credential or secret.
- The value itself remains hidden.
- Usage records support audit and troubleshooting.

## Out of scope

- Displaying raw secret values.
- Granting access to hidden work through usage metadata.

## Related

- B-0104
- B-0125
- B-0126
