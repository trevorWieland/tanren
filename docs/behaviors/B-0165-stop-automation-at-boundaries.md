---
schema: tanren.behavior.v0
id: B-0165
title: Stop automation when user-set boundaries are crossed
area: autonomy-controls
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can rely on Tanren to stop automation when configured boundaries are crossed so automated work stays under control.

## Preconditions

- Boundaries have been configured for scope, cost, time, risk, access, or product intent.
- Tanren is running or preparing automated work affected by those boundaries.

## Observable outcomes

- Work pauses or stops when a boundary is reached.
- The user can see which boundary was crossed and what work was affected.
- Resuming or overriding the boundary requires the configured permission or approval.

## Out of scope

- Treating boundaries as hard-coded persona capabilities.
- Continuing work while hiding boundary violations.

## Related

- B-0104
- B-0162
- B-0163
