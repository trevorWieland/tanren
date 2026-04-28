---
id: B-0138
title: Connect identity and source-control providers
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can connect identity and source-control providers so Tanren can attribute work and reach repositories.

## Preconditions

- The user has permission to add provider connections for the active account or project.
- The provider supports a Tanren-compatible authorization flow.

## Observable outcomes

- The connected providers are visible without exposing secret tokens.
- Tanren can show which repositories or identities are available through each provider.
- Provider connection failures leave actionable, non-secret error information.

## Out of scope

- Managing organization-wide identity policy.
- Bypassing provider approval screens or authorization rules.

## Related

- B-0025
- B-0125
- B-0136
