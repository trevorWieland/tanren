---
schema: tanren.behavior.v0
id: B-0226
title: See provider connection health
area: integration-management
personas: [team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see provider connection health so integration failures are visible before they silently block work.

## Preconditions

- The selected scope has external provider connections visible to the user.

## Observable outcomes

- Each connection reports healthy, degraded, expired, unavailable, or misconfigured state.
- Health explanations identify provider, scope, and likely action without exposing secrets.
- Affected Tanren work links to the unhealthy connection where visible.

## Out of scope

- Polling providers in a way that violates configured limits.
- Showing hidden provider resources through health checks.

## Related

- B-0101
- B-0228
- B-0230
