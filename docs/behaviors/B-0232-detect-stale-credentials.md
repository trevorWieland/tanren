---
schema: tanren.behavior.v0
id: B-0232
title: Detect expiring, unused, or stale credentials
area: configuration
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can detect expiring, unused, or stale credentials so access problems and unused risk can be handled deliberately.

## Preconditions

- Credential or provider metadata is visible to the user.

## Observable outcomes

- Tanren identifies credentials that are near expiration, unused, stale, or no longer matched to active work.
- The view distinguishes user-owned, project-owned, organization-owned, service-account, and worker-scoped access.
- Suggested actions include rotate, revoke, renew, investigate, or accept risk where policy allows.

## Out of scope

- Revealing secret values.
- Revoking credentials without user or policy approval.

## Related

- B-0128
- B-0129
- B-0222
