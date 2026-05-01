---
schema: tanren.behavior.v0
id: B-0128
title: Rotate a credential or secret
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with permission can rotate a credential or secret so Tanren can continue working after credential changes without exposing old or new values.

## Preconditions

- The credential or secret exists.
- The user has permission to update it.

## Observable outcomes

- The new value replaces the prior value for future use.
- The rotation is attributed and recorded without exposing either value.
- Users can see that rotation occurred.

## Out of scope

- Recovering forgotten secret values.
- Rotating external-provider credentials automatically without authorization.

## Related

- B-0125
- B-0126
- B-0129
