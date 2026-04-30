---
schema: tanren.behavior.v0
id: B-0199
title: Distinguish personal credentials from shared secrets
area: configuration
personas: [team-builder, operator, observer]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can distinguish personal credentials from shared project or organization secrets so access boundaries are understandable without exposing secret values.

## Preconditions

- The project or organization uses credentials or secrets.
- The user has visibility into credential or secret metadata.

## Observable outcomes

- Tanren identifies whether access material is user-tier, project-tier, or organization-tier.
- Secret values remain hidden while scope, owner class, usage, and last-updated metadata remain visible where allowed.
- Work that needs unavailable access explains which scope of credential or secret is missing.

## Out of scope

- Displaying secret values after storage.
- Treating shared secrets as personal credentials.

## Related

- B-0048
- B-0125
- B-0126
