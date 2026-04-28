---
id: B-0048
title: Manage user-tier configuration and credentials
area: configuration
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can manage configuration and credentials tied specifically to them,
such as personal authentication tokens for agent providers, so that their
own identity and preferences travel with them without being visible to
teammates.

## Preconditions

- The user is signed into an account.

## Observable outcomes

- The user can view, set, update, and remove user-tier configuration values
  including personal credentials.
- User-tier values are scoped to the individual account — no teammate,
  organization admin, or project member sees another user's user-tier
  values.
- User-tier values take effect immediately for work the user initiates and
  remain available across devices signed into the same account (see
  B-0051).

## Out of scope

- Shared team credentials — those live at the project or organization tier.
- Auditing user-tier values on other people's behalf.

## Related

- B-0049
- B-0050
- B-0051
