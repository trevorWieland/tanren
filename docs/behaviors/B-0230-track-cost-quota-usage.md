---
schema: tanren.behavior.v0
id: B-0230
title: Track cost and quota usage across providers
area: operations
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can track cost and quota usage across harnesses and providers so Tanren-controlled work stays within expected operating limits.

## Preconditions

- The selected scope has configured budget, quota, harness, or provider usage signals visible to the user.

## Observable outcomes

- Usage is grouped by scope, provider, harness, project, or time window where source signals support it.
- Warnings identify budget or quota risk before work fails when possible.
- Missing provider usage data is visible rather than treated as zero usage.

## Out of scope

- Guaranteeing provider billing accuracy.
- Exposing costs from scopes the user cannot see.

## Related

- B-0083
- B-0207
- B-0226
