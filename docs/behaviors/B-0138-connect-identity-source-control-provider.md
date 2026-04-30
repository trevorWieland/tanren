---
schema: tanren.behavior.v0
id: B-0138
title: Select first-run identity and source-control providers
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can select identity and source-control
providers during first-run setup so Tanren can attribute work and reach the
repositories needed for an initial project.

## Preconditions

- The user is completing first-run setup or creating an initial project.
- The user has permission to add provider connections for the active account or project.
- The provider supports a Tanren-compatible authorization flow.

## Observable outcomes

- The selected providers are visible without exposing secret tokens.
- Tanren can show which identities and repositories are available for first-run project setup.
- Provider connection failures leave actionable, non-secret error information and do not block unrelated setup choices.

## Out of scope

- Managing organization-wide identity policy.
- Managing provider integrations after first-run setup.
- Bypassing provider approval screens or authorization rules.

## Related

- B-0025
- B-0125
- B-0136
- B-0223
- B-0224
