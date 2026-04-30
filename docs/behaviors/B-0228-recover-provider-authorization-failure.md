---
schema: tanren.behavior.v0
id: B-0228
title: Recover from provider authorization failure
area: integration-management
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can recover from expired, revoked, or denied provider authorization so affected Tanren work can resume safely.

## Preconditions

- A visible provider connection has an authorization failure.
- The user has permission to refresh, replace, or route recovery for that connection.

## Observable outcomes

- Tanren explains which connection failed and which work is affected.
- Recovery preserves ownership mode, scope, and audit history.
- Work blocked by the failure can retry or resume only after authorization is restored.

## Out of scope

- Silently swapping personal access for shared access.
- Continuing provider actions with stale credentials.

## Related

- B-0107
- B-0128
- B-0226
